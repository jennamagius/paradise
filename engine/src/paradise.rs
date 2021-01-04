pub struct Paradise {
    i: ::std::sync::Arc<Internal>,
}

struct Internal {
    config: crate::config::Config,
    db_pool: ::sqlx::Pool<::sqlx::Postgres>,
    sq: ::parking_lot::Mutex<::iou::SubmissionQueue<'static>>,
    cq: ::parking_lot::Mutex<::iou::CompletionQueue<'static>>,
    registrar: ::iou::Registrar<'static>,
    binds: ::parking_lot::Mutex<Vec<::std::os::unix::io::RawFd>>,
    done: ::std::sync::atomic::AtomicBool,
}

impl Paradise {
    pub async fn new(config: crate::config::Config) -> anyhow::Result<Self> {
        let db_pool = match ::sqlx::pool::Pool::connect(&config.db).await {
            Ok(x) => {
                log::debug!("Database connection established.");
                x
            }
            Err(err) => {
                log::error!("Database connection failed: {}.", err);
                return Err(anyhow::Error::new(err));
            }
        };
        let uring = Box::new(::iou::IoUring::new(16)?);
        let uring = Box::leak(uring);
        let (sq, cq, registrar) = uring.queues();
        let sq = ::parking_lot::Mutex::new(sq);
        let cq = ::parking_lot::Mutex::new(cq);
        let done = false.into();
        let i = Internal {
            config,
            db_pool,
            sq,
            cq,
            registrar,
            done,
            binds: Default::default(),
        };
        let i = ::std::sync::Arc::new(i);
        let result = Paradise { i };
        result.init_binds()?;
        Ok(result)
    }

    fn init_binds(&self) -> ::anyhow::Result<()> {
        for addr in &self.i.config.bind {
            let af = match addr {
                ::std::net::SocketAddr::V4(_) => ::nix::sys::socket::AddressFamily::Inet,
                ::std::net::SocketAddr::V6(_) => ::nix::sys::socket::AddressFamily::Inet6,
            };
            let socket = ::nix::sys::socket::socket(
                af,
                ::nix::sys::socket::SockType::Stream,
                ::nix::sys::socket::SockFlag::SOCK_CLOEXEC,
                ::nix::sys::socket::SockProtocol::Tcp,
            )?;
            let addr = ::nix::sys::socket::InetAddr::from_std(&addr);
            let addr = ::nix::sys::socket::SockAddr::Inet(addr);
            ::nix::sys::socket::bind(socket, &addr)?;
            ::nix::sys::socket::listen(socket, 5)?;
            let mut sq = self.i.sq.lock();
            let mut event = match sq.prepare_sqe() {
                Some(x) => x,
                None => ::anyhow::bail!("Out of submission queue events"),
            };
            let mut token = crate::token::Token::default();
            token.set_category(crate::token::Category::BindAccept);
            let mut binds_lock = self.i.binds.lock();
            let idx: u64 = std::convert::TryFrom::try_from(binds_lock.len())?;
            token.set_idx(idx)?;
            log::trace!("Setting user data to {}.", token.0);
            unsafe { event.prep_accept(socket, None, ::iou::sqe::SockFlag::SOCK_CLOEXEC) };
            unsafe { event.set_user_data(token.0) };
            binds_lock.push(socket);
        }
        self.i.sq.lock().submit()?;

        log::trace!("Binds finished");
        Ok(())
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        ::tokio::task::block_in_place(|| {
            let mut cq = self.i.cq.lock();
            loop {
                log::trace!("Waiting for event.");
                let event = cq.wait_for_cqe();
                log::trace!("Got an event.");
                let event = match event {
                    Ok(x) => x,
                    Err(err) => {
                        log::warn!("Event error: {}.", err);
                        continue;
                    }
                };
                let token = crate::token::Token(event.user_data());
                log::trace!(
                    "Event token: {} ({}, {}).",
                    token.0,
                    token.category(),
                    token.idx()
                );
                match self.dispatch_event(token, event.result()) {
                    Ok(_) => (),
                    Err(err) => log::debug!("Error during event dispatch: {}.", err),
                };
            }
        });
        Ok(())
    }

    fn dispatch_event(
        &self,
        token: crate::token::Token,
        result: Result<u32, std::io::Error>,
    ) -> ::anyhow::Result<()> {
        let category = token.category();
        let category = <crate::token::Category as ::num_traits::FromPrimitive>::from_u64(category);
        let category = match category {
            Some(x) => x,
            None => anyhow::bail!("Unknown event category"),
        };
        match category {
            crate::token::Category::Single => Ok(()), //todo!(),
            crate::token::Category::BindAccept => self.handle_bind_accept(token, result),
        }
    }

    fn handle_bind_accept(
        &self,
        token: crate::token::Token,
        result: Result<u32, std::io::Error>,
    ) -> ::anyhow::Result<()> {
        let idx = token.idx();
        let idx: usize = match ::std::convert::TryFrom::try_from(idx) {
            Ok(x) => x,
            Err(_) => panic!("Unexpected integer overflow."),
        };
        let mut sq = self.i.sq.lock();
        let mut event = match sq.prepare_sqe() {
            Some(x) => x,
            None => panic!("Out of submission queue events."),
        };
        let binds_lock = self.i.binds.lock();
        let socket = match binds_lock.get(idx) {
            Some(x) => x,
            None => {
                anyhow::bail!("Accept event for unknown socket {}.", idx);
            }
        };
        unsafe { event.prep_accept(*socket, None, ::iou::sqe::SockFlag::SOCK_CLOEXEC) };
        unsafe { event.set_user_data(token.0) };
        let result = match result {
            Ok(x) => x,
            Err(err) => {
                log::debug!("Error during accept: {}.", err);
                return Err(::anyhow::Error::new(err));
            }
        };
        let mut event2 = match sq.prepare_sqe() {
            Some(x) => x,
            None => panic!("Out of submission queue events."),
        };
        unsafe { event2.prep_write(result as ::std::os::unix::io::RawFd, &b"asdf\n"[..], 0) };
        sq.submit()?;
        Ok(())
    }
}

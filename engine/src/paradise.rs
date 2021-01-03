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
            unsafe { event.prep_accept(socket, None, ::iou::sqe::SockFlag::SOCK_CLOEXEC) };
            self.i.binds.lock().push(socket);
        }
        self.i.sq.lock().submit();
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
                log::trace!("Event user data: {}.", event.user_data());
            }
        });
        Ok(())
    }
}

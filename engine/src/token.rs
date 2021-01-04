#[derive(Default, Clone, Copy, Debug)]
pub struct Token(pub u64);

impl Token {
    pub fn category(self) -> u64 {
        let mut bytes = [0u8; 8];
        bytes[6..].copy_from_slice(&self.0.to_be_bytes()[..2]);
        u64::from_be_bytes(bytes)
    }

    pub fn idx(self) -> u64 {
        let mut bytes = [0u8; 8];
        bytes[2..].copy_from_slice(&self.0.to_be_bytes()[2..]);
        u64::from_be_bytes(bytes)
    }

    pub fn set_idx(&mut self, idx: u64) -> anyhow::Result<()> {
        if &idx.to_be_bytes()[..2] != &[0, 0] {
            anyhow::bail!("idx must be < 2^48");
        }
        let mut bytes = self.0.to_be_bytes();
        bytes[2..].copy_from_slice(&idx.to_be_bytes()[2..]);
        self.0 = u64::from_be_bytes(bytes);
        Ok(())
    }

    pub fn set_category(&mut self, category: Category) {
        let category = match ::num_traits::ToPrimitive::to_u16(&category) {
            Some(x) => x,
            None => unreachable!("Added more than 65536 categories?"),
        };
        self.set_category_raw(category);
    }

    pub fn set_category_raw(&mut self, category: u16) {
        let mut bytes = self.0.to_be_bytes();
        bytes[..2].copy_from_slice(&category.to_be_bytes());
        self.0 = u64::from_be_bytes(bytes);
    }
}

#[derive(num_derive::FromPrimitive, num_derive::ToPrimitive)]
pub enum Category {
    Single,
    BindAccept,
}

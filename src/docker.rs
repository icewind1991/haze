use color_eyre::Result;

pub struct Docker {
    handle: bollard::Docker,
}

impl Docker {
    pub fn new() -> Result<Self> {
        Ok(Docker {
            handle: bollard::Docker::connect_with_local_defaults()?,
        })
    }
}

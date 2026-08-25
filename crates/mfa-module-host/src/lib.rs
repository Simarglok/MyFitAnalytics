pub mod error;
pub mod package;
pub mod registry;
pub mod store;

pub use error::PackageError;
pub use package::{InspectedEntry, InspectedPackage, InstalledModule, PackageInstaller};
pub use registry::ModuleRegistry;

/// This module contains the domain logic for the application data transfer.
///
/// DTOs are used to transfer data between the application layers. As a best
/// practice, the domain layer should not depend on any other layer, so the DTOs
/// itself. This way, the domain layer can be reused in other crates without
/// any dependencies.
///
pub mod dtos;

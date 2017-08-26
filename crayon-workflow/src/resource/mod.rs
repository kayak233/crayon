pub mod metadata;
pub mod database;

pub mod texture;
pub mod bytes;
pub mod atlas;
pub mod shader;

pub use self::database::ResourceDatabase;
pub use self::metadata::ResourceMetadata;

pub use self::texture::TextureMetadata;
pub use self::bytes::BytesMetadata;
pub use self::atlas::AtlasMetadata;
pub use self::shader::ShaderMetadata;

pub use crayon::resource::workflow::ResourcePayload;
/// The enumeration of all the fundamental resources that could be imported into
/// workspace.
pub use crayon::resource::workflow::ResourcePayload as Resource;

const METADATA_EXTENSION: &'static str = "meta";
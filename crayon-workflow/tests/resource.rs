extern crate crayon;
extern crate crayon_workflow;
extern crate image;

use crayon_workflow::resource;
use crayon_workflow::Manifest;

use std::path::Path;

#[test]
fn database() {
    use resource::ResourceDatabase;

    ///
    let manifest = Manifest::find("tests/workspace").unwrap().setup().unwrap();
    let mut database = ResourceDatabase::new(manifest).unwrap();
    database.refresh().unwrap();
    database.save().unwrap();

    ///
    {
        let path = Path::new("tests/workspace/resources/texture.png.meta");
        assert!(path.exists());

        let path = Path::new("tests/workspace/resources/invalid_texture.png.meta");
        assert!(path.exists());
    }

    /// Make sure processed resources could be read at runtime.
    database
        .build("0.0.1",
               crayon_workflow::platform::BuildTarget::MacOS,
               "tests/build")
        .unwrap();

    let mut rs = crayon::resource::ResourceSystem::new().unwrap();

    {
        rs.load_manifest("tests/build/manifest").unwrap();

        let _: crayon::resource::TextureItem = rs.load("texture.png").unwrap();
        let _: crayon::resource::BytesItem = rs.load("invalid_texture.png").unwrap();
        assert!(rs.load::<crayon::resource::Texture, &str>("invalid_texture.png")
                    .is_err());
    }

    {
        let uuid = database
            .uuid("tests/workspace/resources/texture.png")
            .unwrap();

        rs.load_with_uuid::<crayon::resource::Texture>(uuid)
            .unwrap();
    }

    {
        let atlas: crayon::resource::AtlasItem = rs.load("atlas.json").unwrap();
        let uuid = atlas.read().unwrap().texture();
        rs.load_with_uuid::<crayon::resource::Texture>(uuid)
            .unwrap();
    }
}
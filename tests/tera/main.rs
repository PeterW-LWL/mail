extern crate mail_codec_composition as compose;
extern crate mail_codec as mail;
extern crate futures;
#[macro_use]
extern crate serde_derive;
extern crate futures_cpupool;

use futures::Future;

use compose::composition_prelude::Result;
use compose::composition_prelude::*;
use compose::resource_prelude::*;
use compose::default_impl::{NoNameComposer};
use compose::render_template_engine::{RenderTemplateEngine, DEFAULT_SETTINGS};
use compose::tera::TeraRenderEngine;

#[derive(Serialize)]
struct Name {
    name: &'static str
}

#[test]
fn use_tera_template_a() {
    let tera = TeraRenderEngine::new("./test_resources/tera_base/**/*").unwrap();
    let mut rte = RenderTemplateEngine::new(tera);
    rte.load_specs_from_dir("./test_resources/templates", &*DEFAULT_SETTINGS).unwrap();

    let context = self::tmp_context::SimpleContext::new( "company_a.not_a_domain".into() );
    let composer = Compositor::new( rte, context.clone(), NoNameComposer );

    let data = Name { name: "abcdefg" };

    let from_to = MailSendContext::new(
        Email::try_from( "a@b.c" ).unwrap().into(),
        Email::try_from( "d@e.f" ).unwrap().into(),
        "Dear randomness".into()
    );

    let mail = composer.compose_mail(from_to, "template_a", data).unwrap();

    let mut encoder = Encoder::new( MailType::Ascii );
    let encodable_mail = mail.into_encodeable_mail( &context ).wait().unwrap();
    encodable_mail.encode( &mut encoder ).unwrap();


    println!( "{}", encoder.into_string_lossy().unwrap() );



}

//this will just be a temporary solution until default_impl::SimpleContext is improved
mod tmp_context {

    use std::sync::Arc;
    use std::fmt;
    use std::path::Path;
    use std::borrow::Cow;
    use std::fs::File;
    use std::io::Read;

    use futures::{future, Future};
    use futures_cpupool::{ CpuPool, Builder };

    use mail::prelude::*;
    use mail::utils::SendBoxFuture;
    use mail::context::{ FileLoader, RunElsewhere, CompositeBuilderContext };
    use mail::default_impl::VFSFileLoader;

    use compose::ContentIdGen;
    use compose::default_impl::RandomContentId;




    #[derive(Debug, Clone)]
    pub struct SimpleContext( Arc<SimpleContextInner> );

    struct SimpleContextInner {
        cpu_pool: CpuPool,
        content_id_gen: RandomContentId
    }

    impl SimpleContext {

        pub fn new( content_id_postfix: String ) -> Self {
            SimpleContext(Arc::new(SimpleContextInner {
                cpu_pool: Builder::new().create(),
                content_id_gen: RandomContentId::new(content_id_postfix)
            }))
        }

    }

    impl fmt::Debug for SimpleContextInner {
        fn fmt( &self, fter: &mut fmt::Formatter ) -> fmt::Result {
            fter.debug_struct( "SimpleContext" )
                .field( "content_id_gen", &self.content_id_gen )
                .field( "elsewher", &"CpuPool { .. }" )
                .finish()
        }
    }

    impl FileLoader for SimpleContext {
        type FileFuture = future::FutureResult<Vec<u8>, Error>;

        fn load_file( &self, path: Cow<'static, Path> ) -> Self::FileFuture {
            load_file_fn(path).into()
        }
    }

    fn load_file_fn( path: Cow<'static, Path> ) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut file  = File::open( path )?;
        file.read_to_end( &mut buf )?;
        Ok( buf )
    }


    impl RunElsewhere for SimpleContext {
        fn execute<F>( &self, fut: F) -> SendBoxFuture<F::Item, F::Error>
            where F: Future + Send + 'static,
                  F::Item: Send+'static,
                  F::Error: Send+'static
        {
            self.0.cpu_pool.execute( fut )
        }
    }

    impl ContentIdGen for SimpleContext {
        fn new_content_id(&self) -> Result<MessageID> {
            self.0.content_id_gen.new_content_id()
        }
    }

}
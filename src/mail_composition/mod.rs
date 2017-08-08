use std::marker::PhantomData;

use rand;
use rand::Rng;
use ascii::AsciiStr;

use error::*;
use headers::Header;
use headers::Header::*;

use components::{
    Disposition,
    Unstructured,
    Mailbox, MailboxList,
    Phrase
};

use mail::mime::{
    MultipartMime
};
use mail::resource::{
    Resource
};
use mail::{
    Mail,
    Builder, BuilderContext
};

use data::FromInput;

use self::data::{
    preprocess_data
};
use self::context::{
    Context,
    MailSendContext
};
use self::templates::{
    Template,
    TemplateEngine
};


pub use self::data::{
    EmbeddingInData, AttachmentInData,
    DataInterface,
};
pub use self::resource::{
    EmbeddingInMail, AttachmentInMail,
    Embeddings, Attachments
};


pub mod context;
pub mod templates;
mod resource;
mod data;


pub trait NameComposer<D> {
    fn compose_name( &self, data: &D ) -> Option<String>;
}

pub type BodyWithEmbeddings = (Resource, Embeddings);


pub struct Compositor<T, C, CP, D> {
    template_engine: T,
    context: C,
    name_composer: CP,
    _d: PhantomData<D>
}


impl<T, C, CP, D> Compositor<T, C, CP, D>
    where T: TemplateEngine,
          C: Context,
          CP: NameComposer<D>,
          D: DataInterface
{
    pub fn new( template_engine: T, context: C, name_composer: CP ) -> Self {
        Compositor { template_engine, context, name_composer, _d: PhantomData }
    }

    pub fn builder( &self ) -> Builder<C> {
        Builder( self.context.clone() )
    }

    /// composes a mail based on the given template_id, data and send_context
    pub fn compose_mail( &self,
                         send_context: MailSendContext,
                         data: D,
                         template_id: T::TemplateId
    ) -> Result<Mail> {

        let mut data = data;
        //compose display name => create Address with display name;
        let ( subject, from_mailbox, to_mailbox ) =
            self.preprocess_send_context( send_context, &mut data )?;

        let core_headers = vec![
            From( MailboxList::from_single( from_mailbox ) ),
            To( MailboxList::from_single( to_mailbox ) ),
            Subject( subject )
            //TODO: what else? MessageId? Signature? ... or is it added by relay
        ];

        let ( embeddings, mut attachments ) = self.preprocess_data( &mut data );

        let ( bodies, extracted_attachments ) =
            self.preprocess_templates(
                TemplateEngine::templates( &self.template_engine,
                                           &self.context, template_id, data)?.into() );

        attachments.extend( extracted_attachments );

        self.build_mail( bodies, embeddings, attachments, core_headers )
    }

    /// converts To into a mailbox by composing a display name if nessesary,
    /// and converts the String subject into a "Unstructured" text
    /// returns (subjcet, from_mail, to_mail)
    pub fn preprocess_send_context( &self, sctx: MailSendContext, data: &mut D )
        -> Result<(Unstructured, Mailbox, Mailbox)>
    {
        let from_mailbox = sctx.from;
        let to_mailbox = {
            let mut to_mailbox = sctx.to;
            if to_mailbox.display_name.is_none() {
                if let Some( new_name ) = self.name_composer.compose_name( data ) {
                    let phrase = Phrase::from_input( new_name )?;
                    to_mailbox.display_name = Some( phrase );
                }
            }
            to_mailbox
        };
        let subject = Unstructured::from_input( sctx.subject )?;
        data.see_from_mailbox( &from_mailbox );
        data.see_to_mailbox( &to_mailbox );
        Ok( ( subject, from_mailbox, to_mailbox ) )
    }

    /// Preprocesses the data moving attachments out of it and replacing
    /// embeddings with a ContentID created for them
    /// returns the extracted embeddings and attchments
    pub fn preprocess_data( &self, data: &mut D ) -> (Embeddings, Attachments) {
        preprocess_data( &self.context, data )
    }

    /// maps all alternate bodies (templates) to
    /// 1. a single list of attachments as they are not body specific
    /// 2. a list of Resource+Embedding pair representing the different (sub-) bodies
    pub fn preprocess_templates( &self, templates: Vec<Template> )
        -> (Vec<BodyWithEmbeddings>, Attachments)
    {
        let mut bodies = Vec::new();
        let mut attachments = Vec::new();
        for template in templates {
            bodies.push( (template.body, template.embeddings) );
            attachments.extend( template.attachments );
        }
        (bodies, attachments)
    }


    /// uses the results of preprocessing data and templates, as well as a list of
    /// mail headers like `From`,`To`, etc. to create a new mail
    pub fn build_mail( &self,
                       bodies: Vec<BodyWithEmbeddings>,
                       embeddings: Embeddings,
                       attachments: Attachments,
                       core_headers: Vec<Header>
    ) -> Result<Mail> {
        let bb = self.builder();
        let mail = match attachments.len() {
            0 => bb.create_alternate_bodies_with_embeddings(bodies, embeddings, core_headers )?,
            _n => bb.create_with_attachments(
                |bb| bb.create_alternate_bodies_with_embeddings(bodies, embeddings, Vec::new() ),
                attachments,
                core_headers
            )?
        };
        Ok( mail )
    }
}




pub trait BuilderExt {

    fn create_alternate_bodies(
        &self,
        bodies: Vec<BodyWithEmbeddings>,
        header: Vec<Header>
    ) -> Result<Mail>;

    fn create_alternate_bodies_with_embeddings(
        &self,
        bodies: Vec<BodyWithEmbeddings>,
        embeddings: Embeddings,
        header: Vec<Header>
    ) -> Result<Mail>;

    fn create_mail_body(
        &self,
        body: BodyWithEmbeddings,
        headers: Vec<Header>
    ) -> Result<Mail>;

    fn create_with_attachments<FN>(
        &self,
        body: FN,
        attachments: Attachments,
        headers: Vec<Header>
    ) -> Result<Mail> where FN: FnOnce( &Self ) -> Result<Mail>;

    fn create_body_from_resource(
        &self,
        resource: Resource,
        headers: Vec<Header>
    ) -> Result<Mail>;

    fn create_body_with_embeddings<FN>(
        &self,
        sub_body: FN,
        embeddings: Embeddings,
        headers: Vec<Header>
    ) -> Result<Mail> where FN: FnOnce( &Self ) -> Result<Mail>;

}



impl<E: BuilderContext> BuilderExt for Builder<E> {

    fn create_alternate_bodies(
        &self,
        bodies: Vec<BodyWithEmbeddings>,
        headers: Vec<Header>
    ) -> Result<Mail> {
        let mut bodies = bodies;

        match bodies.len() {
            0 => bail!( ErrorKind::NeedPlainAndOrHtmlMailBody ),
            1 => return self.create_mail_body(bodies.pop().unwrap(), headers ),
            _n => {}
        }

        let mut builder = self
            .multipart( gen_multipart_mime( ascii_str!{ a l t e r n a t e })? )
            .set_headers( headers )?;

        for body in bodies {
            builder = builder.add_body( |bb| bb.create_mail_body( body, Vec::new() ) )?;
        }

        builder.build()
    }

    fn create_alternate_bodies_with_embeddings(
        &self,
        bodies: Vec<BodyWithEmbeddings>,
        embeddings: Embeddings,
        headers: Vec<Header>
    ) -> Result<Mail> {
        match embeddings.len() {
            0 => {
                self.create_alternate_bodies( bodies, headers )
            },
            _n => {
                self.create_body_with_embeddings(
                    |bb| bb.create_alternate_bodies( bodies, Vec::new() ),
                    embeddings,
                    headers
                )
            }
        }
    }

    fn create_mail_body(&self, body: BodyWithEmbeddings, headers: Vec<Header> ) -> Result<Mail> {
        let (resource, embeddings) = body;
        if embeddings.len() > 0 {
            self.create_body_with_embeddings(
                |bb| bb.create_body_from_resource( resource, Vec::new() ),
                embeddings,
                headers
            )
        } else {
            self.create_body_from_resource( resource, headers )
        }
    }

    fn create_body_from_resource( &self, resource: Resource, headers: Vec<Header> ) -> Result<Mail> {
        self.singlepart( resource )
            .set_headers( headers )?
            .build()
    }

    fn create_body_with_embeddings<FN>(
        &self,
        sub_body: FN,
        embeddings: Embeddings,
        headers: Vec<Header>
    ) -> Result<Mail> where FN: FnOnce( &Self ) -> Result<Mail>
    {
        if embeddings.len() == 0 {
            bail!( "this function except at last one embedding" )
        }

        let mut builder = self
            .multipart( gen_multipart_mime( ascii_str!{ r e l a t e d } )? )
            .set_headers( headers )?;


        builder = builder.add_body( sub_body )?;
        for embedding in embeddings {
            let EmbeddingInMail { content_id, resource } = embedding;
            builder = builder.add_body( |b|
                b.create_body_from_resource( resource , vec![
                    Header::ContentID( content_id ),
                    Header::ContentDisposition( Disposition::inline() )
                ])
            )?;
        }
        builder.build()
    }


    fn create_with_attachments<FN>(
        &self,
        body: FN,
        attachments: Vec<AttachmentInMail>,
        headers: Vec<Header>
    )  -> Result<Mail>
        where FN: FnOnce( &Self ) -> Result<Mail>
    {
        let mut builder = self.multipart( gen_multipart_mime( ascii_str!{ m i x e d } )? )
                          .set_headers( headers )?
                          .add_body( body )?;

        for attachment in attachments {
            builder = builder.add_body( |b| b.create_body_from_resource(
                attachment,
                vec![
                    Header::ContentDisposition( Disposition::attachment() )
                ]
            ))?;
        }

        builder.build()
    }
}



fn gen_multipart_mime( subtype: &AsciiStr ) -> Result<MultipartMime> {
    use components::mime::MimeFromStrError;
    //TODO check if subtype is a "valide" type e.g. no " " in ot

    const MULTIPART_BOUNDARY_LENGTH: usize = 30;
    static CHARS: &[char] = &[
        '!',      '#', '$', '%', '&', '\'', '(',
        ')', '*', '+', ',',      '.', '/', '0',
        '1', '2', '3', '4', '5', '6', '7', '8',
        '9', ':', ';', '<', '=', '>', '?', '@',
        'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H',
        'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P',
        'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X',
        'Y', 'Z', '[',      ']', '^', '_', '`',
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h',
        'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p',
        'q', 'r', 's', 't', 'u', 'v', 'w', 'x',
        'y', 'z', '{', '|', '}', '~'
    ];


    // we add =_^ to the boundary, as =_^ is neither valide in base64 nor quoted-printable
    let mut mime_string = format!( "multipart/{}; boundary=\"=_^", subtype );
    let mut rng = rand::thread_rng();
    for _ in 0..MULTIPART_BOUNDARY_LENGTH {
        mime_string.push( CHARS[ rng.gen_range( 0, CHARS.len() )] )
    }
    mime_string.push('"');

    MultipartMime::new(
        //can happen if subtype is invalid
        mime_string.parse()
            .map_err( |err| MimeFromStrError( err ) )
            .chain_err(|| ErrorKind::GeneratingMimeFailed )?
    ).chain_err( || ErrorKind::GeneratingMimeFailed )
}




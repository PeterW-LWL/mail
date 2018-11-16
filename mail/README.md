# mail / mail-api &emsp;

Documentation can be [viewed on docs.rs](https://docs.rs/mail-api). (at least once it's published ;=) )

Facade which re-exports functionality from a number of mail related crates.

The crates include:

- `mail-internals` some parts used by more or less all other `mail-*` crates like
  `MailType`, some grammar parts or the `EncodingBuffer`. As `mail-internals` is
  mainly used internally it's not directly exposed.
- `mail-headers` functionality wrt. mail (mime) header, including a HeaderMap
  containing the header fields (keeping insertion order) a number of components
  used in header field bodies like e.g. `Mailbox` or `Phrase` and default
  implementations for many headers, including From, To, Sender, Data, Subject,
  Date, MessageId, Cc, Bcc, etc.
- `mail-core` provides the type `Mail` which represents a (possible)
  multi-part mime Mail and includes functionality for encoding it. It also
  contains an abstraction for the content of multi-part mails called
  `Resource`, which includes support for embeddings, attachments etc.
- `mail-template` provides functionality to create a `Mail` from a template,
  including multi-part mails containing embeddings and attachments. It's not
  bound to a specific template engine. Currently bindings for the tera template
  engine are provided behind feature flag.
- `mail-smtp` (feature: `smtp`) provides bindings between `mail-core` and
  `new-tokio-smtp` allowing the simple sending of mails to a specific server.
  It's mainly focused on the use-case where mails are sent to an Mail
  Submission Agent (MSA) which then distributes them
- `mail-render-template-engine` (feature `render-template-engine`) provides a
  partial implementation for the `TemplateEngine` trait from `mail-template`
  only missing a "render engine" to render the template. The implementation
  includes functionality for automatically generating multiple alternate bodies
  (e.g. text, html) embedding, and attachments based on a spec, which can be
  derived and loaded from a folder/file layout making it easy to create and
  maintain complex mail templates.

## Examples

### [`mail_by_hand`](./examples/mail_by_hand.rs)

Creates and encodes a simple mail without using any fancy helpers, templates or
similar.

### [`mail_from_template`](./examples/mail_from_template/main.rs)

Uses the bindings for the `tera` template engine to create a mail, including
alternate bodies and an attachment.

### [`send_mail`](./examples/send_mail/main.rs)

A simple program which queries the user for information and then sends a
(simple) mail to an MSA (Mail Submission Agent).  While it is currently limited
to STARTTLS on port 587, Auth Plain and only simple text mails this is a
limitation of this cli program not the mail libraries which can handle other
forms of connecting and authenticating etc.

Note that this is meant to send data to an MSA NOT a MX (Mail Exchanger), e.g.
`smtp.gmail.com` is a MSA but `gmail-smtp-in.l.google.com` is an MX.  Also note
that some mail providers do not accept Auth Plain (at least not without
enabling it in the security settings). The reason for this is that they prefer
that applications do not use username+password for authentication but other
formats e.g. OAuth2 tokens.

Rest assured that the authentication data is only sent over a TLS encrypted
channel. Still if you don't trust it consider using some throw away or testing
mail service e.g. `ethereal.email`.

Lastly the examples uses the same unique seed every time, which means that
Message-ID's, and Content-ID's are not guaranteed to be world unique even
through they should (again a limitation of the example not the mail crate).
Nevertheless given that it also doesn't use its "own" domain but a `.test`
domain it can't guarantee world uniqueness anyway.

## Features

### `smtp`

Provides bindings to `new-tokio-smtp` under `mail::smtp` by reexporting the
`mail-smtp` crate

### `render-template-engine`

Provides the render template engine under `mail::render_template_engine`.

### `askama-engine`

Provides bindings to the `askama` crate (a template engine) under
`mail::askama`

### `tera-engine`

Provides bindings to the `tera` crate (a template engine) under `mail::tera`.
This feature uses the `render-template-engine` feature.

### `traceing`

Enables the `traceing` debugging functionality in the `EncodingBuffer`
from `mail-internals`, this is only used for testing header implementations
and comes with noticeable overhead. **As such this should not be enabled
except for testing header implementations**. Also `EncodingBuffer` isn't
re-exported as it can be seen as an internal part of the implementation
which normally doesn't need to be accessed directly.

## License

Licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

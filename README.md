
# mail-template

**Provides mechanisms for generating mails based on templates**

---

This crate provides a general interface for using template engine with the mail crate.

It's core is the `TemplateEngine` trait which can be implemented to bind a template engine.
When rendering a template the template engine implementing the `TemplateEngine` trait will
produce a number of (wrapped) `Resource` instances representing the alternate bodies of amail as well as a number of additional `Resources` used for embedded content (e.g. logoimages) and attachments. This crate then takes this parts and composes a multipart mime mail from
it.

## Template Engine implementations

A mail template engine has to do more then just taking a single text
template (e.g. a handlebars template) and produce some output using
string magic. It has to:

1. consider alternate bodies, so it should render at last two
   "text templates" (one text/plain, one html)

2. consider which additional embeddings/attachments should be included
   (all in the given template data are included, but you might
    add additional ones, e.g. some logo image)

As such text template engines like `Handle` bar can not directly
be bound to the `TemplateEngine` trait.

For using text template engine it's recommended to use
the `mail-template-render-engine` (also exposed through the
mail facade) which implements this overhead for any engine
which can "just" render some text and provides default
bindings to some common template engines (e.g. Handlebars).

## Derive

This crate requires template data to implement `InspectEmbeddedResources`
which combined with some typing/generic design decisions allows to bind
not just to template engines which use serialization to access template
data but also to such which use static typing (like `askama`).

As such it re-exports the `InspectEmbeddedResources` derive from
`mail-derive`. Note that if you use the mail facade it also does
re-export the derive.

## Features

- `askama-engine`, includes bindings for the askama template engine.
- `serialize-to-content-id`, implements Serialize for `Embedded`,
   `EmbeddedWithCId` which serializes the embedded type **into its
   content id**. E.g. a image with content id `"q09cu3@example.com"`
   will be serialized to the string `"q09cu3@example.com"`. This is
   extremely useful for all template engines which use serialization
   as their way to access template data.

## Example

```rust
```

## Road Map

The current implementation has a number of limitations which should be lifted with
future versions:

- Only a limited subset of headers are/can be set through the template engine
  (`Sender`, `From`, `To`, `Subject`) while some headers are set implicitly
  when encoding the mail (e.g. `Date`, `Content-Type`, `Content-Disposition`).
  But sometimes it would be useful to add some custom headers through the template
  engine (both on the outermost and inner bodies).
- `From`, `To`, `Subject` have to be given, but sometimes you might want to just
  create the `Mail` type and then set them by yourself (through you _can_ currently
  override them)
- Re-use/integration of existing mail instances: Some times you might want to
  use a `Mail` instance created some where else as a body for a multipart mail
  generated from a template (e.g. some thing generating "special" attachments).

Also there are some parts which are likely to change:

- `MailSendData`/`MailSendDataBuilder` the name is
  not very good it also needs to change to handle
  the thinks listed above
- `Embedded`, `EmbeddedWithCid`, embeddings and attachments
  currently a `Embedded` instance is a wrapper around `Resource`
  representing something which will become a mail body but is not
  a main body (i.e. it not the text/html/.. you send) instead it
  something embedded in the mail which is either used as attachment
  or as a embedding (e.g. a logo image). Through the content disposition
  the `Embedded` instance differs between thing embedded and internally
  used or embedded and used as attachment at the same time many different
  arrays are sometimes used to differ between them (e.g. in `MailParts`)
  but there is no (type system) check to make sure a array of thinks used
  as attachments can not contain a `Embedded` instance with content disposition
  inline. The result will still be a valid mail, but likely not in the
  way you expect it to be. This should be fixed one way or another (making
  the different array type safe and lifting disposition to the type level
  had been used but didn't play out nicely).
- `serialize-to-content-id`, is a nice idea but has some problems in
  some edge cases (e.g. when you want to serialize anything containing
  a `Embedded` type for any usage BUT accessing it in an template). So
  it might be removed, which means a import like `cid:{{data.mything}}`
  (using some mustach pseudo template syntax) would become `cid:{{data.mything.cid}}`.


## Documentation


Documentation can be [viewed on docs.rs](https://docs.rs/mail-template).
(once published)

## License

Licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

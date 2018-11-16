
# mail-headers

**Provides header specific functionality for the `mail` crate**

---

This crate provides header specific functionality for the `mail`
crate. This includes:

- `HeaderName`, `Header` and `HeaderMap` as for the general API
- `HeaderTryFrom`, `HeaderTryInto` as the `TryFrom`/`TryInto`
  traits are not stable but we need something similar to their
  functionality.
- a number of headers like `_To`,`_From`, `Sender`, `Subject`
  and many more (Note that `_To` and `_From` are prefixed with
  and `_` to prevent name collisions when importing them, i.e.
  importing `_From as From` would shadow `std::convert::From`
  which can lead to extremely irritating errors).
- a number of components which are used to represent the
  content/field body of an header field e.g. `MailboxList`
  or `Email`. They are placed in the `components` module.
- a `headers!` macro for making the creation of an `HeaderMap`
  with a number of headers easier.
- a `def_headers!` macro for defining new custom headers

## Example (HeaderMap)

A header map is a collection representing a number
of mail headers in an specific order. It can be
created like this:

```rust
#[macro_use]
extern crate mail_headers;

// just import all headers
use mail_headers::*;
use mail_headers::error::ComponentCreationError;

fn create_headers() -> Result<HeaderMap, ComponentCreationError> {
    headers!{
        // from and to can have multiple values
        // until specialization is stable is array
        // is necessary
        _From: [("My Fancy Display Name", "theduck@example.com")],
        _To: [ "unknown@example.com", ],
        Subject: "Who are you?"
    }
}

fn main() {
    let headers = create_headers().unwrap();
    assert_eq!(headers.len(), 3);

    if let Some(subject) = headers.get_single(Subject) {
        // as long a you don't play around with custom headers AND
        // don't mix multiple implementations for the same header
        // `.unwrap()` is just fine.
        let subject = subject.expect("mixed different Subject header implementations");
        println!("found subject: {}", subject);
    }
}
```

## Example (custom header)

If needed users of the `mail` crate can create their own
headers, through this should be done with care.

Note that the second field (then `unchecked { <name> }`),
expects a specific naming scheme, the auto-generated test
do check if it's violated but if you just run the code and
ignore the failing tests strange error can occure. (
The scheme is a capitalise the first letter of each
word and write all other letter in lower case, i.e.
`X-Id` is ok but `X-ID` isn't). The reason for this is because
of the way the header does the field lookup. While this
is not nice, for most use cases there is no need to
generate custom headers and in the future this might be
circumvented by auto-generating the name with a proc-derive.

```rust
#[macro_use]
extern crate mail_headers;

use mail_headers::components;

// this will define two headers `XFooEmail` and `XBarMailbox`
// the first will add a header field named `X-Foo-Email` with
// a value which is an `components::Email` and the second will
// add field with a value which is an `components::Mailbox`.
//
// Note that through the way they are defined `XFooEmail` can
// at most appear 1 time in an header map, while `XBarMailbox`
// can appear multiple times. Be aware that this is checked through
// so called validators which needs to be explicitly run, which they
// are if this header map is used to create a mail (running them
// when adding fields would be a mess as you would have to add
// transactions which can add/remove multiple fields at once, and
// implementing auto-generation for some fields which are required if
// some other fields are given in a certain way would be harder too).

// If in scope both can be used in the `headers!` macro,
// like any other header.
//
def_headers! {
    // the name of the auto-generated test
    test_name: validate_header_names,

    // the scope from which all components should be imported
    // E.g. `DateTime` refers to `components::DateTime`.
    scope: components,

    // definitions of the headers or the form
    // <type_name>, unchecked { <filed_name> }, <component>, <validator>
    XFooEmail, unchecked { "X-Foo-Email"      }, Email ,   maxOne,
    XBarMailbox, unchecked { "X-Bar-Mailbox" }, Mailbox, None
}

fn main() {
    let headers = headers! {
        XFooEmail: "123@example.com",
        XBarMailbox: ("My Funy Name", "notfunny@example.com"),
        XBarMailbox: "without.display.name@example.com"
    }.unwrap();
}
```


## Documentation

Documentation can be [viewed on docs.rs](https://docs.rs/mail-headers).
(once it is published ;) )


## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any
additional terms or conditions.

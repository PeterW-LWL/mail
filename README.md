**Work on this crate has been suspended, it might be continued in the future.**
(If you are interested in forking and like to have some questions/tipps/opinons about
 how it can make sense to continue this crate feel free to contact me.)

# mail

This repository contains the rust mail crates.

They provides ways to represent, generate and
send mails.

The generation can be done with custom code or
using a template library. Bindings for `handlebars`
are included but binding other libraries isn't
hard either.

The sending is done over `SMTP` it is currently
focused on sending the mails to a
Message Submission Agent (MSA) through it could
be used in other contexts, too.

Currently there is no mail parsing implemented.

The readme of the `mail` crate which acts as
a facade exposing all this features can be
fund under [`mail/README.md`](mail/README.md)

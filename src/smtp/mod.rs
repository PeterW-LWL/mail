use std::net::SocketAddr;
use serde::Serialize;

use tokio_smtp;
use tokio_smtp::request::{ClientId as SmtpClientId};
use tokio_smtp::client::ClientParams;

use ::context::BuilderContext;

// 1. I want to provide a service, which accepts a struct containing From+To+Subject+Data+TemplateId
// 2. the setup needs to know the Context as well as setup for an Smtp connection

use ::context::Context;


pub enum SmtpServerInfo {
    //Domain(String),
    Address(SocketAddr)
}

pub struct ConnectionSecurity {
    sni_domain: String,
    tls_setup_method: TlsSetupMethod,
    //TODO add support to hook in to the `TlsConnector::builder().map(|bder| bder.build())` step
}

pub enum TlsSetupMethod {
    STARTTLS,
    DirectTLS
}

#[derive(Clone)]
pub struct SmtpSetup<C: Context> {
    context: C,
    //potentially arc this parts
    smtp_client_id: SmtpClientId,
    smtp_server_info:  SmtpServerInfo,
    connection_security: ConnectionSecurity
}


pub struct MailSendData<D: Serialize> {

}
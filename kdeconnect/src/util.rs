use std::{
    future::Future,
    net::{Ipv4Addr, SocketAddrV4},
    sync::Arc,
    time::Duration,
};

use log::info;
use rcgen::{Certificate, CertificateParams, DnType, KeyPair};
use time::OffsetDateTime;
use tokio::{
    io::{AsyncRead, AsyncWriteExt},
    net::TcpListener,
};
use tokio_rustls::{
    rustls::{
        self,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider},
        pki_types::{CertificateDer, ServerName, UnixTime},
        server::{
            danger::{ClientCertVerified, ClientCertVerifier},
        },
        DigitallySignedStruct, ServerConfig,
    },
    TlsAcceptor,
};
use x509_parser::{certificate::X509Certificate, der_parser::asn1_rs::FromDer};

use crate::KdeConnectError;

pub(crate) fn generate_server_cert(
    keypair: &KeyPair,
    uuid: &str,
) -> Result<Certificate, rcgen::Error> {
    // just in case also add to domain name
    let mut params = CertificateParams::new([uuid.to_string()])?;
    // KDE Connect Android does it like this
    let now = OffsetDateTime::now_utc();
    params.not_before = now - Duration::from_days(365);
    params.distinguished_name.push(DnType::CommonName, uuid);
    params
        .distinguished_name
        .push(DnType::OrganizationName, "r58Playz");
    params
        .distinguished_name
        .push(DnType::OrganizationalUnitName, "kdeconnectjb");
    params.self_signed(keypair)
}

pub(crate) fn get_public_key(cert: &[u8]) -> Result<Vec<u8>, KdeConnectError> {
    Ok(X509Certificate::from_der(cert)?.1.public_key().raw.to_vec())
}

#[derive(Debug)]
pub(crate) struct NoCertificateVerification(CryptoProvider);

impl NoCertificateVerification {
    pub fn new(provider: CryptoProvider) -> Self {
        Self(provider)
    }
}

impl ServerCertVerifier for NoCertificateVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

impl ClientCertVerifier for NoCertificateVerification {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        &[]
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }
}

pub(crate) fn get_time_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .expect("time went backwards")
        .as_millis()
}

pub(crate) async fn create_payload(
    payload: impl AsyncRead + Sync + Send + Unpin,
    server_config: Arc<ServerConfig>,
) -> Result<(u16, impl Future<Output = ()> + Sync + Send), KdeConnectError> {
    let mut free_listener: Option<TcpListener> = None;
    let mut free_port: Option<u16> = None;
    for port in 60000..=64000 {
        if let Ok(listener) =
            TcpListener::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port)).await
        {
            free_listener = Some(listener);
            free_port = Some(port);
            break;
        }
    }
    if let Some(free_listener) = free_listener
        && let Some(free_port) = free_port
    {
        Ok((free_port, async move {
            let mut payload = payload;
            if let Ok(incoming) = free_listener.accept().await
                && let Ok(mut stream) = TlsAcceptor::from(server_config).accept(incoming.0).await
                && let Ok(_) = tokio::io::copy(&mut payload, &mut stream).await
                && let Ok(_) = stream.flush().await
            {
                let _ = stream.shutdown().await;
            }
            info!("successfully sent payload on port {}", free_port)
        }))
    } else {
        Err(KdeConnectError::NoPayloadTransferPortFound)
    }
}

// Copyright 2020 Alex Dukhno
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::{
    hand_shake,
    messages::{BackendMessage, Encryption},
    tests::{
        async_io::{empty_file_named, TestCase},
        certificate_content, pg_frontend,
    },
    ProtocolConfiguration,
};
use futures_lite::future::block_on;
use std::{
    io::Write,
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    path::PathBuf,
};

fn path_to_temp_certificate() -> PathBuf {
    let named_temp_file = empty_file_named();
    let mut file = named_temp_file.reopen().expect("file with content");
    file.write_all(&certificate_content())
        .expect("write certificate content to temp file");
    named_temp_file.path().to_path_buf()
}

#[test]
fn trying_read_from_empty_stream() {
    block_on(async {
        let test_case = TestCase::with_content(vec![]);

        let config = ProtocolConfiguration::none();

        let result = hand_shake(
            test_case,
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)),
            &config,
        )
        .await;

        assert!(result.is_err());
    });
}

#[test]
fn trying_read_only_length_of_ssl_message() {
    block_on(async {
        let test_case = TestCase::with_content(vec![&[0, 0, 0, 8], &[]]);

        let config = ProtocolConfiguration::none();

        let result = hand_shake(
            test_case,
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)),
            &config,
        )
        .await;

        assert!(result.is_err());
    });
}

#[test]
fn sending_reject_notification_for_none_secure() {
    block_on(async {
        let test_case = TestCase::with_content(vec![pg_frontend::Message::SslRequired.as_vec().as_slice(), &[]]);

        let config = ProtocolConfiguration::none();

        let result = hand_shake(
            test_case.clone(),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)),
            &config,
        )
        .await;

        assert!(result.is_err());

        let actual_content = test_case.read_result().await;
        let mut expected_content = Vec::new();
        expected_content.extend_from_slice(Encryption::RejectSsl.into());
        assert_eq!(actual_content, expected_content);
    });
}

#[test]
fn sending_accept_notification_for_ssl_only_secure() {
    block_on(async {
        let test_case = TestCase::with_content(vec![pg_frontend::Message::SslRequired.as_vec().as_slice(), &[]]);

        let config = ProtocolConfiguration::with_ssl(path_to_temp_certificate(), "password".to_owned());

        let result = hand_shake(
            test_case.clone(),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)),
            &config,
        )
        .await;

        assert!(result.is_err());

        let actual_content = test_case.read_result().await;
        let mut expected_content = Vec::new();
        expected_content.extend_from_slice(Encryption::AcceptSsl.into());
        assert_eq!(actual_content, expected_content);
    });
}

#[test]
fn successful_connection_handshake_for_none_secure() {
    block_on(async {
        let test_case = TestCase::with_content(vec![
            pg_frontend::Message::SslRequired.as_vec().as_slice(),
            pg_frontend::Message::Setup(vec![
                ("user", "username"),
                ("database", "database_name"),
                ("application_name", "psql"),
                ("client_encoding", "UTF8"),
            ])
            .as_vec()
            .as_slice(),
            pg_frontend::Message::Password("123").as_vec().as_slice(),
            &[],
        ]);

        let config = ProtocolConfiguration::none();

        let result = hand_shake(
            test_case.clone(),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)),
            &config,
        )
        .await;

        assert!(result.is_ok());

        let actual_content = test_case.read_result().await;
        let mut expected_content = Vec::new();
        expected_content.extend_from_slice(Encryption::RejectSsl.into());
        expected_content.extend_from_slice(BackendMessage::AuthenticationCleartextPassword.as_vec().as_slice());
        expected_content.extend_from_slice(BackendMessage::AuthenticationOk.as_vec().as_slice());
        expected_content.extend_from_slice(
            BackendMessage::ParameterStatus("client_encoding".to_owned(), "UTF8".to_owned())
                .as_vec()
                .as_slice(),
        );
        expected_content.extend_from_slice(
            BackendMessage::ParameterStatus("DateStyle".to_owned(), "ISO".to_owned())
                .as_vec()
                .as_slice(),
        );
        expected_content.extend_from_slice(
            BackendMessage::ParameterStatus("integer_datetimes".to_owned(), "off".to_owned())
                .as_vec()
                .as_slice(),
        );
        assert_eq!(actual_content, expected_content);
    });
}

#[test]
#[ignore] //TODO find work around not to do real SSL handshake
fn successful_connection_handshake_for_ssl_only_secure() {
    block_on(async {
        let test_case = TestCase::with_content(vec![
            pg_frontend::Message::SslRequired.as_vec().as_slice(),
            pg_frontend::Message::Setup(vec![
                ("user", "username"),
                ("database", "database_name"),
                ("application_name", "psql"),
                ("client_encoding", "UTF8"),
            ])
            .as_vec()
            .as_slice(),
            pg_frontend::Message::Password("123").as_vec().as_slice(),
        ]);

        let config = ProtocolConfiguration::with_ssl(path_to_temp_certificate(), "password".to_owned());

        let result = hand_shake(
            test_case.clone(),
            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)),
            &config,
        )
        .await;

        assert!(result.is_ok());

        let actual_content = test_case.read_result().await;
        let mut expected_content = Vec::new();
        expected_content.extend_from_slice(Encryption::AcceptSsl.into());
        expected_content.extend_from_slice(BackendMessage::AuthenticationCleartextPassword.as_vec().as_slice());
        expected_content.extend_from_slice(BackendMessage::AuthenticationOk.as_vec().as_slice());
        expected_content.extend_from_slice(
            BackendMessage::ParameterStatus("client_encoding".to_owned(), "UTF8".to_owned())
                .as_vec()
                .as_slice(),
        );
        expected_content.extend_from_slice(
            BackendMessage::ParameterStatus("DateStyle".to_owned(), "ISO".to_owned())
                .as_vec()
                .as_slice(),
        );
        assert_eq!(actual_content, expected_content);
    });
}

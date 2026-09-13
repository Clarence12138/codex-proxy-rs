use gateway_portal::model::{
    MutationActor, MutationContext, PortalErrorKind,
    auth::{LoginCommand, LoginError},
    users::{CreatePortalUser, ResetPortalPassword},
};
use secrecy::SecretString;

use super::auth::{MemoryAuth, services};

#[tokio::test]
async fn create_and_reset_share_relaxed_password_rules_and_login_byte_limit() {
    let services = services(MemoryAuth::default());
    let context = MutationContext {
        actor: MutationActor::System,
        request_id: "user-password-test".to_owned(),
    };
    let users = services.users();
    let auth = services.auth();
    for password in [
        "12345".to_owned(),
        "字".repeat(5),
        "a".repeat(257),
        "字".repeat(86),
    ] {
        assert_eq!(
            users
                .create(
                    &context,
                    CreatePortalUser {
                        username: "known".to_owned(),
                        password: password.clone()
                    }
                )
                .await
                .unwrap_err()
                .kind(),
            PortalErrorKind::Invalid
        );
        assert_eq!(
            users
                .reset_password(
                    &context,
                    ResetPortalPassword {
                        user_id: "usr_known".to_owned(),
                        password
                    }
                )
                .await
                .unwrap_err()
                .kind(),
            PortalErrorKind::Invalid
        );
    }
    for password in [
        "123456".to_owned(),
        "abcdef".to_owned(),
        "$$$$$$".to_owned(),
        "字".repeat(6),
        "a".repeat(256),
        " ".repeat(6),
    ] {
        let user = users
            .create(
                &context,
                CreatePortalUser {
                    username: "known".to_owned(),
                    password: password.clone(),
                },
            )
            .await
            .unwrap();
        auth.login(LoginCommand {
            username: "known".to_owned(),
            password: SecretString::from(password.clone()),
            client_ip: "127.0.0.1".to_owned(),
        })
        .await
        .unwrap();
        users
            .reset_password(
                &context,
                ResetPortalPassword {
                    user_id: user.id.clone(),
                    password: "temporary-password".to_owned(),
                },
            )
            .await
            .unwrap();
        assert_eq!(
            auth.login(LoginCommand {
                username: "known".to_owned(),
                password: SecretString::from(password.clone()),
                client_ip: "127.0.0.1".to_owned()
            })
            .await
            .unwrap_err(),
            LoginError::InvalidCredentials
        );
        users
            .reset_password(
                &context,
                ResetPortalPassword {
                    user_id: user.id,
                    password: password.clone(),
                },
            )
            .await
            .unwrap();
        auth.login(LoginCommand {
            username: "known".to_owned(),
            password: SecretString::from(password),
            client_ip: "127.0.0.1".to_owned(),
        })
        .await
        .unwrap();
    }
}

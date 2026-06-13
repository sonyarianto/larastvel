use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

#[derive(Debug, thiserror::Error)]
pub enum MailError {
    #[error("Failed to build email: {0}")]
    Build(String),
    #[error("Failed to send email: {0}")]
    Send(String),
    #[error("Mailer error: {0}")]
    General(String),
}

#[derive(Debug, Clone)]
pub struct Mailable {
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub from: Option<String>,
    pub reply_to: Option<String>,
    pub subject: String,
    pub body: String,
    pub content_type: ContentType,
}

impl Mailable {
    pub fn new(to: Vec<String>, subject: &str, body: &str) -> Self {
        Self {
            to,
            cc: Vec::new(),
            bcc: Vec::new(),
            from: None,
            reply_to: None,
            subject: subject.to_string(),
            body: body.to_string(),
            content_type: ContentType::TEXT_PLAIN,
        }
    }

    pub fn html(to: Vec<String>, subject: &str, body: &str) -> Self {
        Self {
            to,
            cc: Vec::new(),
            bcc: Vec::new(),
            from: None,
            reply_to: None,
            subject: subject.to_string(),
            body: body.to_string(),
            content_type: ContentType::TEXT_HTML,
        }
    }

    pub fn from(mut self, from: &str) -> Self {
        self.from = Some(from.to_string());
        self
    }

    pub fn reply_to(mut self, reply_to: &str) -> Self {
        self.reply_to = Some(reply_to.to_string());
        self
    }

    pub fn cc(mut self, addresses: Vec<String>) -> Self {
        self.cc = addresses;
        self
    }

    pub fn bcc(mut self, addresses: Vec<String>) -> Self {
        self.bcc = addresses;
        self
    }
}

#[async_trait]
pub trait Mailer: Send + Sync + std::fmt::Debug {
    async fn send(&self, mailable: Mailable) -> Result<(), MailError>;
    fn name(&self) -> &str;
}

#[derive(Debug, Clone)]
pub struct SmtpMailer {
    name: String,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpMailer {
    pub fn new(
        name: &str,
        host: &str,
        port: u16,
        username: &str,
        password: &str,
    ) -> Result<Self, MailError> {
        let creds = Credentials::new(username.to_string(), password.to_string());
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)
            .map_err(|e| MailError::Build(e.to_string()))?
            .port(port)
            .credentials(creds)
            .build();
        Ok(Self {
            name: name.to_string(),
            transport,
        })
    }

    pub fn new_without_auth(name: &str, host: &str, port: u16) -> Result<Self, MailError> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
            .port(port)
            .build();
        Ok(Self {
            name: name.to_string(),
            transport,
        })
    }

    pub fn relay(name: &str, host: &str) -> Result<Self, MailError> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::relay(host)
            .map_err(|e| MailError::Build(e.to_string()))?
            .build();
        Ok(Self {
            name: name.to_string(),
            transport,
        })
    }
}

#[async_trait]
impl Mailer for SmtpMailer {
    async fn send(&self, mailable: Mailable) -> Result<(), MailError> {
        let from = mailable
            .from
            .clone()
            .unwrap_or_else(|| "larastvel@localhost".to_string());

        let mut message_builder = Message::builder()
            .from(
                from.parse::<lettre::message::Mailbox>()
                    .map_err(|e| MailError::Build(e.to_string()))?,
            )
            .subject(&mailable.subject);

        if mailable.to.is_empty() {
            return Err(MailError::Build("No recipients specified".to_string()));
        }

        for addr in &mailable.to {
            message_builder = message_builder
                .to(
                    addr.parse::<lettre::message::Mailbox>()
                        .map_err(|e| MailError::Build(e.to_string()))?,
                );
        }

        for addr in &mailable.cc {
            message_builder = message_builder
                .cc(
                    addr.parse::<lettre::message::Mailbox>()
                        .map_err(|e| MailError::Build(e.to_string()))?,
                );
        }

        for addr in &mailable.bcc {
            message_builder = message_builder
                .bcc(
                    addr.parse::<lettre::message::Mailbox>()
                        .map_err(|e| MailError::Build(e.to_string()))?,
                );
        }

        if let Some(reply_to) = &mailable.reply_to {
            message_builder = message_builder
                .reply_to(
                    reply_to.parse::<lettre::message::Mailbox>()
                        .map_err(|e| MailError::Build(e.to_string()))?,
                );
        }

        let message = message_builder
            .header(mailable.content_type)
            .body(mailable.body)
            .map_err(|e| MailError::Build(e.to_string()))?;

        self.transport
            .send(message)
            .await
            .map_err(|e| MailError::Send(e.to_string()))?;

        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct LogMailer {
    name: String,
}

impl LogMailer {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
}

#[async_trait]
impl Mailer for LogMailer {
    async fn send(&self, mailable: Mailable) -> Result<(), MailError> {
        tracing::info!(
            target: "larastvel::mail",
            "📧 [{}] To: {:?} | Subject: {} | Body: {}",
            self.name,
            mailable.to,
            mailable.subject,
            if mailable.body.len() > 200 {
                format!("{}... ({} bytes)", &mailable.body[..200], mailable.body.len())
            } else {
                mailable.body.clone()
            },
        );
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone)]
pub struct MailManager {
    mailers: HashMap<String, Arc<dyn Mailer>>,
    default: String,
}

impl MailManager {
    pub fn new(default: &str) -> Self {
        Self {
            mailers: HashMap::new(),
            default: default.to_string(),
        }
    }

    pub fn register<M: Mailer + 'static>(&mut self, name: &str, mailer: M) {
        self.mailers.insert(name.to_string(), Arc::new(mailer));
    }

    pub fn mailer(&self, name: &str) -> Result<Arc<dyn Mailer>, MailError> {
        self.mailers
            .get(name)
            .cloned()
            .ok_or_else(|| MailError::General(format!("Mailer [{}] not configured", name)))
    }

    pub fn default_mailer(&self) -> Result<Arc<dyn Mailer>, MailError> {
        self.mailer(&self.default)
    }

    pub fn set_default(&mut self, name: &str) {
        self.default = name.to_string();
    }

    pub fn default_name(&self) -> &str {
        &self.default
    }

    pub fn mailer_names(&self) -> Vec<String> {
        self.mailers.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_log_mailer_send() {
        let mailer = LogMailer::new("log");
        let mailable = Mailable::new(
            vec!["test@example.com".to_string()],
            "Test Subject",
            "Hello, this is a test email body.",
        );
        let result = mailer.send(mailable).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_mailer_html() {
        let mailer = LogMailer::new("log");
        let mailable = Mailable::html(
            vec!["user@example.com".to_string()],
            "HTML Email",
            "<h1>Hello</h1><p>World</p>",
        );
        let result = mailer.send(mailable).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_mailer_name() {
        let mailer = LogMailer::new("my-mailer");
        assert_eq!(mailer.name(), "my-mailer");
    }

    #[tokio::test]
    async fn test_log_mailer_multiple_recipients() {
        let mailer = LogMailer::new("log");
        let mailable = Mailable::new(
            vec![
                "alice@example.com".to_string(),
                "bob@example.com".to_string(),
            ],
            "Multiple",
            "To multiple recipients",
        );
        let result = mailer.send(mailable).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_log_mailer_with_cc_bcc() {
        let mailer = LogMailer::new("log");
        let mailable = Mailable::new(
            vec!["to@example.com".to_string()],
            "CC Test",
            "With CC and BCC",
        )
        .cc(vec!["cc@example.com".to_string()])
        .bcc(vec!["bcc@example.com".to_string()])
        .from("sender@example.com")
        .reply_to("reply@example.com");

        let result = mailer.send(mailable).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mailable_builder() {
        let m = Mailable::new(
            vec!["a@b.com".to_string()],
            "Subject",
            "Body",
        )
        .from("f@b.com")
        .reply_to("r@b.com")
        .cc(vec!["c@b.com".to_string()])
        .bcc(vec!["d@b.com".to_string()]);

        assert_eq!(m.from, Some("f@b.com".to_string()));
        assert_eq!(m.reply_to, Some("r@b.com".to_string()));
        assert!(m.cc.contains(&"c@b.com".to_string()));
        assert!(m.bcc.contains(&"d@b.com".to_string()));
    }

    #[tokio::test]
    async fn test_mail_manager_default() {
        let mut manager = MailManager::new("log");
        manager.register("log", LogMailer::new("log"));
        manager.register("smtp", LogMailer::new("smtp"));

        let default = manager.default_mailer().unwrap();
        assert_eq!(default.name(), "log");

        let mailer = manager.mailer("smtp").unwrap();
        assert_eq!(mailer.name(), "smtp");
    }

    #[tokio::test]
    async fn test_mail_manager_missing_mailer() {
        let manager = MailManager::new("log");
        let result = manager.mailer("nonexistent");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_mail_manager_names() {
        let mut manager = MailManager::new("log");
        manager.register("log", LogMailer::new("log"));
        manager.register("ses", LogMailer::new("ses"));
        let mut names = manager.mailer_names();
        names.sort();
        assert_eq!(names, vec!["log", "ses"]);
    }

    #[tokio::test]
    async fn test_mail_manager_set_default() {
        let mut manager = MailManager::new("log");
        manager.register("log", LogMailer::new("log"));
        manager.register("smtp", LogMailer::new("smtp"));
        manager.set_default("smtp");
        assert_eq!(manager.default_name(), "smtp");

        let default = manager.default_mailer().unwrap();
        assert_eq!(default.name(), "smtp");
    }

    #[tokio::test]
    async fn test_smtp_mailer_creation() {
        let result = SmtpMailer::new("smtp", "localhost", 1025, "user", "pass");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_smtp_mailer_creation_without_auth() {
        let result = SmtpMailer::new_without_auth("smtp", "localhost", 1025);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mailable_no_recipients() {
        let mailer = LogMailer::new("log");
        let mailable = Mailable::new(vec![], "No To", "body");
        let result = mailer.send(mailable).await;
        assert!(result.is_ok(), "LogMailer should not reject empty recipients");
    }

    #[tokio::test]
    async fn test_mailable_html_constructor() {
        let m = Mailable::html(
            vec!["u@e.com".to_string()],
            "HTML",
            "<b>bold</b>",
        );
        assert_eq!(m.content_type, ContentType::TEXT_HTML);
        assert_eq!(m.subject, "HTML");
        assert_eq!(m.body, "<b>bold</b>");
    }

    #[tokio::test]
    async fn test_log_mailer_long_body() {
        let mailer = LogMailer::new("log");
        let long = "a".repeat(500);
        let mailable = Mailable::new(vec!["t@t.com".to_string()], "Long", &long);
        let result = mailer.send(mailable).await;
        assert!(result.is_ok());
    }
}

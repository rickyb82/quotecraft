//! QuoteCraft's hands (layer L1): WordCraft's command line makes the DOCX and the PDF out of
//! the quote's HTML (inserted into a letterhead when there is one), MailCraft's server sends
//! the mail as the user, with the documents attached.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod docx;
pub mod mailcraft;
pub mod wordcraft;

use quotecraft_core::bridge::{RendererFactory, SenderFactory};

/// The renderer factory apps hand to the engine: `wordcraft-cli` from the preferences.
pub fn renderer_factory() -> RendererFactory {
    std::sync::Arc::new(|prefs| Box::new(wordcraft::WordCraft::new(&prefs.wordcraft_cli)))
}

/// The sender factory apps hand to the engine: MailCraft's server from the preferences and
/// the service token from the secrets.
pub fn sender_factory() -> SenderFactory {
    std::sync::Arc::new(|prefs, secrets| {
        mailcraft::MailCraft::new(&prefs.mailcraft_url, &secrets.mailcraft_token).map(|m| Box::new(m) as Box<dyn quotecraft_core::bridge::Sender>)
    })
}

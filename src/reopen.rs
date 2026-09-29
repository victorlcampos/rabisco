//! Abrir o Rabisco pelo Finder, Spotlight ou Launchpad enquanto ele já está rodando
//! faz o macOS mandar o evento "reopen" ao processo existente; aqui ele abre o editor.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_foundation::NSAppleEventManager;
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

pub struct Ivars {
    proxy: EventLoopProxy<UserEvent>,
}

define_class!(
    // SAFETY: NSObject não tem requisitos para subclasses e não implementamos Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RabiscoReopenHandler"]
    #[ivars = Ivars]
    pub struct ReopenHandler;

    impl ReopenHandler {
        #[unsafe(method(handleReopen:withReplyEvent:))]
        fn handle_reopen(&self, _event: &AnyObject, _reply: &AnyObject) {
            let _ = self.ivars().proxy.send_event(UserEvent::OpenEditor);
        }
    }
);

const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const REOPEN_APPLICATION: u32 = u32::from_be_bytes(*b"rapp");

/// Instala o tratador. O retorno precisa ficar vivo: o NSAppleEventManager não o retém.
pub fn install(proxy: EventLoopProxy<UserEvent>) -> Option<Retained<ReopenHandler>> {
    let mtm = MainThreadMarker::new()?;
    let this = ReopenHandler::alloc(mtm).set_ivars(Ivars { proxy });
    let handler: Retained<ReopenHandler> = unsafe { msg_send![super(this), init] };
    let manager = NSAppleEventManager::sharedAppleEventManager();
    unsafe {
        let _: () = msg_send![
            &*manager,
            setEventHandler: &*handler,
            andSelector: sel!(handleReopen:withReplyEvent:),
            forEventClass: CORE_EVENT_CLASS,
            andEventID: REOPEN_APPLICATION
        ];
    }
    Some(handler)
}

#[macro_export]
macro_rules! popup_error {
    ($msg:expr) => {{
        let error_msg = $msg.to_string();
        eprintln!("{}", error_msg);
        #[cfg(not(target_os = "android"))]
        let _ = msgbox::create("Game Error", &error_msg, msgbox::IconType::Error);
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        let error_msg = format!($fmt, $($arg)*);
        eprintln!("{}", error_msg);
        #[cfg(not(target_os = "android"))]
        let _ = msgbox::create("Game Error", &error_msg, msgbox::IconType::Error);
    }};
}

#[macro_export]
macro_rules! popup_bail {
    ($msg:expr) => {{
        let error_msg = $msg.to_string();
        #[cfg(not(target_os = "android"))]
        let _ = msgbox::create("Game Bail", &error_msg, msgbox::IconType::Error);
        anyhow::bail!("{}", error_msg);
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        let error_msg = format!($fmt, $($arg)*);
        #[cfg(not(target_os = "android"))]
        let _ = msgbox::create("Game Bail", &error_msg, msgbox::IconType::Error);
        anyhow::bail!("{}", error_msg);
    }};
}

#[macro_export]
macro_rules! popup_panic {
    ($msg:expr) => {{
        let error_msg = $msg.to_string();
        #[cfg(not(target_os = "android"))]
        let _ = msgbox::create("Game Panic", &error_msg, msgbox::IconType::Error);
        panic!("{}", error_msg);
    }};
    ($fmt:expr, $($arg:tt)*) => {{
        let error_msg = format!($fmt, $($arg)*);
        #[cfg(not(target_os = "android"))]
        let _ = msgbox::create("Game Panic", &error_msg, msgbox::IconType::Error);
        panic!("{}", error_msg);
    }};
}

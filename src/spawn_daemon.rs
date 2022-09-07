use nix::unistd::{fork, setsid, ForkResult};

/// Defines which branch of the fork we are on
pub enum DaemonResult {
    Main,
    Daemon,
}

/// Spawn a daemon by double forking the current process
pub fn spawn_daemon() -> DaemonResult {
    match unsafe { fork().expect("Failed to fork process") } {
        ForkResult::Parent { child } => {
            // wait for the fork child in order to prevent it from becoming a zombie
            nix::sys::wait::waitpid(Some(child), None).unwrap();
            DaemonResult::Main
        }

        ForkResult::Child => {
            // make the child the leader, so it does not get killed when parent receive sigterm
            setsid().expect("Failed to make the child the session leader");

            // double fork to ensure we spawn a daemon that can't acquire a controlling terminal
            if let ForkResult::Child =
                unsafe { nix::unistd::fork().expect("Failed to fork child process") }
            {
                return DaemonResult::Daemon;
            }

            // exit from the first child
            std::process::exit(0)
        }
    }
}

extern crate getopts;

extern crate otp_cop;

use std::{env, thread};
use std::sync::{mpsc, Arc};

use otp_cop::service::{CreateServiceResult, ServiceFactory};


struct ParallelIter<T> {
    count: usize,
    pos: usize,
    rx: mpsc::Receiver<T>
}

impl<T> Iterator for ParallelIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if self.pos < self.count {
            self.count += 1;
            return self.rx.recv().ok();
        } else {
            return None;
        }
    }
}

fn parallel<T, U, F1>(objs: Vec<T>, f1: F1) -> ParallelIter<U>
        where F1: 'static + Fn(T) -> U + Send + Sync, T: 'static + Send, U: 'static + Send {
    let (tx, rx) = mpsc::channel();
    let count = objs.len();
    let shared_f1 = Arc::new(f1);
    for o in objs {
        let f1 = shared_f1.clone();
        let tx = tx.clone();
        thread::spawn(move || {
            tx.send(f1(o)).unwrap();
        });
    }

    return ParallelIter{count: count, pos: 0, rx: rx};
}

fn main() {
    let service_factories = vec![
        Box::new(otp_cop::SlackServiceFactory) as Box<ServiceFactory>,
        Box::new(otp_cop::GithubServiceFactory) as Box<ServiceFactory>,
    ];

    let mut opts = getopts::Options::new();

    for factory in service_factories.iter() {
        factory.add_options(&mut opts);
    }

    let matches = match opts.parse(env::args().skip(1)) {
        Ok(matches) => matches,
        Err(e) => panic!(e.to_string()),
    };

    let mut services = vec![];

    for factory in service_factories.iter() {
        match factory.create_service(&matches) {
            CreateServiceResult::Service(s) => services.push(s),
            CreateServiceResult::MissingArguments(arg) => panic!(format!("Missing arguments: {:?}", arg)),
            CreateServiceResult::None => continue,
        }
    }

    if services.is_empty() {
        print!("{}", opts.usage("otp-cop: <args>"));
    }

    let count = services.len();
    for (i, result) in parallel(services, |service| service.get_users()).enumerate() {
        match result {
            Ok(result) => {
                let header = format!("{} ({})", result.service_name, result.users.len());
                println!("{}", header);
                println!("{}", "=".chars().cycle().take(header.len()).collect::<String>());
                println!("");
                for user in result.users {
                    let email = match user.email {
                        Some(email) => format!(" ({})", email),
                        None => "".to_string(),
                    };
                    let details = match user.details {
                        Some(details) => format!(" -- {}", details),
                        None => "".to_string(),
                    };
                    println!("@{}{}{}", user.name, email, details);
                }
                if i + 1 != count {
                    println!("");
                    println!("");
                }
            },
            Err(e) => {
                println!("{}", e.service_name);
                println!("{}", "=".chars().cycle().take(e.service_name.len()).collect::<String>());
                println!("");
                println!("{}", e.error_message);
            }
        }
    }
}

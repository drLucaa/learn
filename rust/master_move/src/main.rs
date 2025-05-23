use std::sync::{Arc, Mutex};

fn c<F: FnOnce() + 'static>(f: F) {
    f();
}

fn main() {
    let mut x = vec![1, 2, 3];
    x.push(4);
    let last = x.last().unwrap();
    println!("{:?}", last);

    let v = Arc::new(Mutex::new(vec![1, 2, 3]));

    c({
        let v = v.clone();
        move || {
            println!("inner 1: {:?}", v);
            v.lock().unwrap().push(4);
        }
    });

    c({
        let v = v.clone();
        move || {
            println!("inner 2: {:?}", v);
            v.lock().unwrap().push(5);
        }
    });

    println!("outer: {:?}", v)
}

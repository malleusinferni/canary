extern crate canary;

macro_rules! generate {
    ( $name:ident ) => {
        #[test]
        fn $name() {
            let path = format!("tests/{}.cy", stringify!($name));
            let mut env = canary::compile(path.as_ref()).unwrap();
            env.exec("main", &[]).unwrap();
        }
    }
}

generate!(while_loops);
generate!(arrays);

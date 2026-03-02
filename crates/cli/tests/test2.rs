use std::env;

mod hello;

#[test]
fn test_in_custom_folder_integration() {
    if let Ok(value) = env::var("SIMPLEX_TEST_RUN") {
        println!("hello");
    } else {
        return;
    }

    assert_eq!(2 + 2, 4);
}

#[test]
fn test_in_custom_folder2_integration() {
    if let Ok(value) = env::var("SIMPLEX_TEST_RUN") {
        println!("hello");
    } else {
        return;
    }

    assert_eq!(2 + 2, 4);
}

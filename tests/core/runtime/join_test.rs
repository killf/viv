use std::future::Future;
use std::pin::Pin;
use viv::core::runtime::{block_on_local, join, join_all};

#[test]
fn join_two_futures() {
    let result = block_on_local(Box::pin(async {
        let a = async { 1 + 1 };
        let b = async { "hello" };
        join(Box::pin(a), Box::pin(b)).await
    }));
    assert_eq!(result, (2, "hello"));
}

#[test]
fn join_all_multiple_futures() {
    let result = block_on_local(Box::pin(async {
        let futures: Vec<Pin<Box<dyn Future<Output = i32>>>> = (0..5)
            .map(|i| Box::pin(async move { i * 2 }) as Pin<Box<dyn Future<Output = i32>>>)
            .collect();
        join_all(futures).await
    }));
    assert_eq!(result, vec![0, 2, 4, 6, 8]);
}

#[test]
fn join_all_empty() {
    let result: Vec<i32> = block_on_local(Box::pin(async {
        let futures: Vec<Pin<Box<dyn Future<Output = i32>>>> = vec![];
        join_all(futures).await
    }));
    assert!(result.is_empty());
}

#[test]
fn join_all_single() {
    let result = block_on_local(Box::pin(async {
        let futures: Vec<Pin<Box<dyn Future<Output = &str>>>> = vec![Box::pin(async { "only" })];
        join_all(futures).await
    }));
    assert_eq!(result, vec!["only"]);
}

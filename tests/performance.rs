use std::time::{Duration, Instant};

use service_matrix_rust::algorithm::{MatrixIndex, search_dictionary};

#[test]
fn fifty_by_fifty_algorithm_search_completes_under_one_second() {
    let matrix: Vec<Vec<String>> = (0..50)
        .map(|row| {
            (0..50)
                .map(|column| ((b'a' + ((row + column) % 26) as u8) as char).to_string())
                .collect()
        })
        .collect();
    let index = MatrixIndex::new(&matrix).unwrap();
    let candidates = vec![
        "abcdefghij".to_owned(),
        "zyxwvutsrq".to_owned(),
        "notpresentbecauseofdigits123".to_owned(),
    ];

    let started = Instant::now();
    let _ = search_dictionary(&index, candidates, 3, 30, 50);
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "50x50 algorithm search took {:?}",
        started.elapsed()
    );
}

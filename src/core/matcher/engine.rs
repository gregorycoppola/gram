fn sub_filter_for_label(label: &str) -> KindFilter {
    match label {
        "dp" => KindFilter::Only("dp".to_string()),
        "n" => KindFilter::Only("n".to_string()),
        "s\\agent" => KindFilter::Only("s\\agent".to_string()),
        "s\\patient" => KindFilter::Only("s\\patient".to_string()),
        "s\\theme" => KindFilter::Only("s\\theme".to_string()),
        "s" => KindFilter::Only("s".to_string()),
        _ => KindFilter::Any,
    }
}
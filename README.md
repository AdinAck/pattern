# pattern

A compact pattern recognition and extraction system.

## no_std

This crate is intended for use in `no_std` environments.

# Usage

Given an iterator of some serialized data, values of various types can be extracted in sequence.

```rust
fn get_from_data(data: &mut &[u8]) -> Result<(u8, u16, u16, u32), PatternError> {
    let mut pattern = Pattern::new(data.iter());

    let [start] = pattern
        .values([0u8, 1u8]) // look for these exact values in order
        .deferred() // eat bytes until these values are found
        .extract()?; // attempt extraction

    let [a] = pattern
        .get() // deserialize 1 expected type of a (in this case u8)
        .extract()?;

    let [b, c] = pattern
        .get() // deserialize 2 of the expected type (u16)
        .extract_and(|bytes| { /* do something with the bytes */ })?; // attempt extraction then call closure on bytes extracted

    let [d] = pattern
        .get() // deserialize 1 expected type (u32)
        .extract()?;

    Ok((a, b, c, d))
}
```

# Dependencies

pattern uses the [tiny-serde](https://github.com/AdinAck/tiny-serde) crate for deserialization.

[defmt](https://github.com/knurling-rs/defmt) is available behind a feature gate for formatting `PatternError`.

# Design Considerations

## Safety

This crate *does* use `unsafe` blocks but has been proven to never exhibit undefined behavior.

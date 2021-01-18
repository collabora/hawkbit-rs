# hawkbit_mock

Mock server implementation of [Eclipse hawkBit](https://www.eclipse.org/hawkbit/)
using [httpmock](https://crates.io/crates/httpmock).

This mock is used to test the [hawkbit crate](https://crates.io/crates/hawkbit)
but can also be useful to test any `hawkBit` client.
So far only the [Direct Device Integration API](https://www.eclipse.org/hawkbit/apis/ddi_api/)
is implemented.

## Documentation

See the [crate documentation](https://docs.rs/hawkbit_mock/).
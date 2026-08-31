# Architecture notes

## Actors

- **Producer**: dedicated compute node, spare power capacity, runs DTMSHR. DTMSHR implements RDMA, exposes single system image (SSI) compute node to consumers.
- **Consumer**: existing machine, existing software, offloads CPU to a producer instead of local scaling.

## Open questions

- RDMA transport choice (RoCEv2 / InfiniBand / iWARP / soft-RoCE for dev).
- SSI model: process migration vs. remote exec vs. full VM.
- Discovery: how consumer finds/selects a producer.
- Trust/auth between producer and consumer.
- Metering: producer's "extra watts" budget, consumer's usage accounting.

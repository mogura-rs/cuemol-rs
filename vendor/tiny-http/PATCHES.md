# Local patch

This directory contains the library sources and manifest of `tiny_http` 0.12.0.
`SOURCE.json` records the upstream revision and registry archive checksum.

The connection pool counts a notified worker as waiting until that worker can
acquire the queue lock. During a burst, several queued connections can therefore
reserve the same worker. Existing HTTP keep-alive connections occupy those
workers indefinitely, leaving later connections unprocessed.

`TaskPool::spawn` now subtracts queued tasks when checking worker availability.
It starts a worker when all waiting workers already have queued work. The public
API and HTTP parsing behavior are unchanged. The CLI's connection-burst test
keeps every client open while requiring a response from every accepted socket.

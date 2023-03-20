# spin-componentize

This library converts a Spin module to a
[component](https://github.com/WebAssembly/component-model/).

See [reactor.wit](adapter/wit/reactor.wit) for the definition of the Spin world.
Note that although the world specifies both `inbound-redis` and `inbound-http`
exports, `spin-componentize` will only export either or both according to what
the original module exported.

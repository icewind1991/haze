    'redis' => [
        'host' => 'tls://127.0.0.1',
        'port' => 6379,
        'ssl_context' => [
           'local_cert' => '/redis-certificates/client.crt',
           'local_pk' => '/redis-certificates/client.key',
           'cafile' => '/redis-certificates/ca.crt',
           'verify_peer_name' => false,
        ],
	  ],
//PLACEHOLDER

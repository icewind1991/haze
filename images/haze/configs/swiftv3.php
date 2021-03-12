    'objectstore' => [
        'class' => 'OC\Files\ObjectStore\Swift',
        'arguments' => [
            // replace with your bucket
            'bucket' => 'nextcloud',
            'autocreate' => true,
            'user' => [
                'name' => 'swift',
                'password' => 'swift',
                'domain' => [
                    'name' => 'default',
                ]
            ],
            'scope' => [
                'project' => [
                    'name' => 'service',
                    'domain' => [
                        'name' => 'default',
                    ],
                ],
            ],
            'tenantName' => 'service',
            'region' => 'regionOne',
            'url' => 'http://keystone:5000/v3',
            'serviceName' => 'swift',
        ],
    ],

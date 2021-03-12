	'objectstore' => [
		'class' => 'OC\Files\ObjectStore\Swift',
		'arguments' => [
			// replace with your bucket
			'bucket' => 'nextcloud',
			'autocreate' => true,
			'username' => 'swift',
            'password' => 'swift',
            'tenantName' => 'service',
            'region' => 'regionOne',
			'url' => 'http://keystone:5000/v2.0',
			'serviceName' => 'swift',
		],
	],

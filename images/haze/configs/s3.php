	'objectstore' => [
		'class' => 'OC\Files\ObjectStore\S3',
		'arguments' => [
			'bucket' => 'nextcloud',
			'autocreate' => true,
			'key'    => 'minio',
			'secret' => 'minio123',
			'hostname' => 's3',
			'port' => 9000,
			'use_ssl' => false,
			'use_path_style' => true,
			'uploadPartSize' => 52428800,
		],
	],

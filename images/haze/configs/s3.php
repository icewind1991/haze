	'objectstore' => [
		'class' => 'OC\Files\ObjectStore\S3',
		'arguments' => [
			// replace with your bucket
			'bucket' => 'nextcloud',
			'autocreate' => true,
			'key'    => 'dummy',
			'secret' => 'dummyj',
			'hostname' => 's3',
			'port' => 4566,
			'use_ssl' => false,
			'use_path_style' => true,
			'uploadPartSize' => 52428800,
		],
	],

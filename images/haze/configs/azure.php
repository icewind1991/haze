    'objectstore' => [
        'class' => 'OC\\Files\\ObjectStore\\Azure',
	    'arguments' => array(
		    'container' => 'test',
    		'account_name' => 'devstoreaccount1',
    		'account_key' => 'Eby8vdM02xNOcqFlqUwJPLlmEtlCDXJ1OUzFT50uSRZ6IFsuFq2UVErCz4I6tq/K1SZFPTOtr/KBHBeksoGMGw==',
    		'endpoint' => 'http://azure:10000/devstoreaccount1',
    		'autocreate' => true
	    )
	],

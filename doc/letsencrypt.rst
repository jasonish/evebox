Lets Encrypt
============

EveBox supports self managing TLS certificates from Lets Encrypt if
the following conditions are met:

* The server can listen on port 443 (automatically set with the
  ``--letsencrypt`` command line option.
* EveBox is reachable publically with a DNS hostname, as required by
  the Acme protocol.

Due to the requirement of being publically reachable this is probably
not useful for most.

Example
-------

Say your EveBox host is reachable at "demo.evebox.org", you would
start EveBox like::

  evebox server --letsencrypt demo.evebox.org

This will start the EveBox server on port 443 with TLS certificates
automatically provisioned from Lets Encrypt.

As this requires listening on port 443, you will need to make sure the
user running EveBox has the ability to bind to port 443.

.. note:: On Linux a program may be given the ability to bind to a
          privileged port by setting the appropriate capability, for
          example::

	    setcap 'cap_net_bind_service=+ep' /usr/bin/evebox

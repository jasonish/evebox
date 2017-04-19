TLS
===

Starting the EveBox Server with TLS
-----------------------------------

Before TLS can be used a private key and certificate must be
obtained. EveBox provides a tool to generate a self signed certificate
if a certificate cannot be obtained through other means.

Enabling TLS on the Command Line
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. option:: --tls
   
   Enables TLS.

.. option:: --tls-cert FILE

   Specify the filename of the TLS certificate file.

.. option:: --tls-key FILE

   Specify the filename of the TLS private key. May be ommitted if the
   certificate file is a bundle containing the key.

Example::

  evebox --tls --tls-cert cert.pem --tls-key key.pem

Enabling TLS in the Configuration File
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

TLS can be enabled in the configuration file under ``http.tls``:

.. code-block:: yaml

   http:
     tls:
       enabled: true
       certificate: /path/to/cert.pem
       key: /path/to/key.pem

Creating a Self Signed Certificate
----------------------------------

EveBox ships with a tool to generate self signed TLS certificates.

Example::

  evebox gencert -o evebox.pem

Full usage of ``evebox gencert``:
  
.. literalinclude:: gencert-usage.txt

Lets Encrypt
------------

EveBox supports self managing TLS certificates from Lets Encrypt if
the following conditions are met:

* The server can listen on port 443 (automatically set with the
  ``--letsencrypt`` command line option.
* EveBox is reachable publically with a DNS hostname, as required by
  the Acme protocol.

Due to the requirement of being publically reachable this is probably
not useful for most users.

Example
~~~~~~~

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

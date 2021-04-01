Authentication
==============

Enabling Authentication
-----------------------

Authentication requires:

* Enabling authentication in your EveBox configuration file::

    authentication:
      required: true
      type: usernamepassword

* And enabling the configuration database either with the ``-D`` command
  line option or the ``data-directory`` configuration file setting.

.. note:: If using the RPM or Debian packages AND starting EveBox with
          systemd, the data-directory is already configured to be
          ``/var/lib/evebox``.

.. note:: For the rest of this documentation, ``/var/lib/evebox`` will
          be used as the data-directory.

Starting the Server with Authentication Enabled
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

If EveBox was installed with a RPM or Debian package and started with
*systemd*, it is already setup with a configuration database, just
enable authentication in ``/etc/evebox/evebox.yaml`` (you may first
need to ``cp /etc/evebox/evebox.yaml.example
/etc/evebox/evebox.yaml``.

Otherwise, if you are manually starting EveBox you must use the ``-D``
command line option to set the data directory where the configuration
database can be stored::

  ./evebox server -D ~/.evebox/

.. note:: The ``EVEBOX_DATA_DIRECTORY`` environment variable can also
          be used to set the data directory.

Adding a User
-------------

Adding users is done with the config tool, for example::

  evebox config -D /var/lib/evebox users add --username joe

.. note:: RPM and Debian package installations of EveBox setup
          `/var/lib/evebox` to be owned by the user ``evebox``, so you
          may need use sudo to add users, for example::

	    sudo -u evebox evebox config -D /var/lib/evebox users add

	  or as root::

	    sudo evebox config -D /var/lib/evebox users add


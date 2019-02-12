EveBox Agent
============

The EveBox "agent" is a tool for sending *eve* events directly to
EveBox without the need for tools like *Filebeat* and/or
*Logstash*. Events sent with the agent are handled by the EveBox
server and stored in the database by the server.

Command Line Options
--------------------

.. literalinclude:: agent-usage.txt

Configuration File
------------------

.. literalinclude:: ../agent.yaml.example
   :language: yaml

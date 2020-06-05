// (Inbox) Alert query with query string.
let inbox_alert_query = {
    "query": {
        "bool": {
            "filter": [
                {"exists": {"field": "event_type"}}, {"term": {"event_type": "alert"}},
                {
                    "query_string": {
                        "default_operator": "AND",
                        "query": "WPAD"
                    }
                },
                {"range": {"@timestamp": {"gte": "2020-03-24T12:50:40.783194632-06:00"}}}],
            "must_not": [{"term": {"tags": "archived"}}]
        }
    },
    "sort": [{"@timestamp": {"order": "desc"}}],
    "aggs": {
        "signatures": {
            "aggs": {
                "sources": {
                    "aggs": {
                        "destinations": {
                            "aggs": {
                                "escalated": {"filter": {"term": {"tags": "escalated"}}},
                                "newest": {"top_hits": {"size": 1, "sort": [{"@timestamp": {"order": "desc"}}]}},
                                "oldest": {"top_hits": {"size": 1, "sort": [{"@timestamp": {"order": "asc"}}]}}
                            }, "terms": {"field": "dest_ip", "size": 10000}
                        }
                    },
                    "terms": {"field": "src_ip", "size": 10000}
                }
            },
            "terms": {"field": "alert.signature_id", "size": 10000}
        }
    }
};

let example_inbox_response = {
    "alerts": [
        {
            "count": 1,
            "event": {
                "_id": "OBBqGnEB063otvX71iCh",
                "_index": "logstash-2020.03.26-000001",
                "_score": null,
                "_source": {
                    "@timestamp": "2020-03-27T05:15:41.499Z",
                    "@version": "1",
                    "alert": {
                        "action": "allowed",
                        "category": "Potentially Bad Traffic",
                        "gid": 1,
                        "metadata": {"created_at": ["2010_09_23"], "updated_at": ["2010_09_23"]},
                        "rev": 7,
                        "severity": 2,
                        "signature": "GPL ATTACK_RESPONSE id check returned root",
                        "signature_id": 2100498
                    },
                    "app_proto": "http",
                    "community_id": "1:/3ZVQ/15jRgWDB1ZPoQZAZwRBRg=",
                    "dest_ip": "10.16.1.11",
                    "dest_port": 40014,
                    "event_type": "alert",
                    "flow": {
                        "bytes_toclient": 567,
                        "bytes_toserver": 419,
                        "pkts_toclient": 4,
                        "pkts_toserver": 5,
                        "start": "2020-03-27T05:15:40.235270+0000"
                    },
                    "flow_id": 1854491917588230,
                    "host": "firewall.codemonkey.net",
                    "http": {
                        "hostname": "www.testmyids.com",
                        "http_content_type": "text/html",
                        "http_method": "GET",
                        "http_response_body": "dWlkPTAocm9vdCkgZ2lkPTAocm9vdCkgZ3JvdXBzPTAocm9vdCkK",
                        "http_response_body_printable": "uid=0(root) gid=0(root) groups=0(root)\n",
                        "http_user_agent": "curl/7.66.0",
                        "length": 39,
                        "protocol": "HTTP/1.1",
                        "status": 200,
                        "url": "/"
                    },
                    "in_iface": "enp4s0f0",
                    "packet": "2MuK7aFGoDafTEwoCABFAAA0V/xAADAG0yQfA/WFChABCwBQnE5SKkUyOROLKIARAONKdgAAAQEICqopFHhQTwSY",
                    "packet_info": {"linktype": 1},
                    "path": "/var/log/suricata/eve.json",
                    "payload": "SFRUUC8xLjEgMjAwIE9LDQpTZXJ2ZXI6IG5naW54LzEuMTYuMQ0KRGF0ZTogRnJpLCAyNyBNYXIgMjAyMCAwNToxNTo0NCBHTVQNCkNvbnRlbnQtVHlwZTogdGV4dC9odG1sOyBjaGFyc2V0PVVURi04DQpDb250ZW50LUxlbmd0aDogMzkNCkNvbm5lY3Rpb246IGtlZXAtYWxpdmUNCkxhc3QtTW9kaWZpZWQ6IEZyaSwgMTAgSmFuIDIwMjAgMjE6MzY6MDIgR01UDQpFVGFnOiAiMjctNTliY2ZlOTkzMmMzMiINCkFjY2VwdC1SYW5nZXM6IGJ5dGVzDQoNCnVpZD0wKHJvb3QpIGdpZD0wKHJvb3QpIGdyb3Vwcz0wKHJvb3QpCg==",
                    "payload_printable": "HTTP/1.1 200 OK\r\nServer: nginx/1.16.1\r\nDate: Fri, 27 Mar 2020 05:15:44 GMT\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: 39\r\nConnection: keep-alive\r\nLast-Modified: Fri, 10 Jan 2020 21:36:02 GMT\r\nETag: \"27-59bcfe9932c32\"\r\nAccept-Ranges: bytes\r\n\r\nuid=0(root) gid=0(root) groups=0(root)\n",
                    "proto": "TCP",
                    "src_ip": "31.3.245.133",
                    "src_port": 80,
                    "stream": 1,
                    "tags": [],
                    "timestamp": "2020-03-27T05:15:40.630357+0000",
                    "type": "eve"
                }, "_type": "_doc", "sort": [1585286141499]
            },
            "maxTs": "2020-03-27T05:15:41.499Z",
            "minTs": "2020-03-27T05:15:41.499Z",
            "escalatedCount": 0
        },
        {
            "count": 1,
            "event": {
                "_id": "NhBqGnEB063otvX71iCh",
                "_index": "logstash-2020.03.26-000001",
                "_score": null,
                "_source": {
                    "@timestamp": "2020-03-27T05:15:41.498Z",
                    "@version": "1",
                    "alert": {
                        "action": "allowed",
                        "category": "Attempted Information Leak",
                        "gid": 1,
                        "metadata": {"created_at": ["2011_06_14"], "updated_at": ["2011_06_14"]},
                        "rev": 4,
                        "severity": 2,
                        "signature": "ET POLICY curl User-Agent Outbound",
                        "signature_id": 2013028
                    },
                    "app_proto": "http",
                    "community_id": "1:/3ZVQ/15jRgWDB1ZPoQZAZwRBRg=",
                    "dest_ip": "31.3.245.133",
                    "dest_port": 80,
                    "event_type": "alert",
                    "flow": {
                        "bytes_toclient": 501,
                        "bytes_toserver": 353,
                        "pkts_toclient": 3,
                        "pkts_toserver": 4,
                        "start": "2020-03-27T05:15:40.235270+0000"
                    },
                    "flow_id": 1854491917588230,
                    "host": "firewall.codemonkey.net",
                    "http": {
                        "hostname": "www.testmyids.com",
                        "http_content_type": "text/html",
                        "http_method": "GET",
                        "http_response_body": "dWlkPTAocm9vdCkgZ2lkPTAocm9vdCkgZ3JvdXBzPTAocm9vdCkK",
                        "http_response_body_printable": "uid=0(root) gid=0(root) groups=0(root)\n",
                        "http_user_agent": "curl/7.66.0",
                        "length": 39,
                        "protocol": "HTTP/1.1",
                        "status": 200,
                        "url": "/"
                    },
                    "in_iface": "enp4s0f0",
                    "packet": "oDafTEwo2MuK7aFGCABFAAA03xVAAEAGPAsKEAELHwP1hZxOAFA5E4snUipFMoAQAfVJ6gAAAQEIClBPBJiqKRP0",
                    "packet_info": {"linktype": 1},
                    "path": "/var/log/suricata/eve.json",
                    "payload": "R0VUIC8gSFRUUC8xLjENCkhvc3Q6IHd3dy50ZXN0bXlpZHMuY29tDQpVc2VyLUFnZW50OiBjdXJsLzcuNjYuMA0KQWNjZXB0OiAqLyoNCg0K",
                    "payload_printable": "GET / HTTP/1.1\r\nHost: www.testmyids.com\r\nUser-Agent: curl/7.66.0\r\nAccept: */*\r\n\r\n",
                    "proto": "TCP",
                    "src_ip": "10.16.1.11",
                    "src_port": 40014,
                    "stream": 1,
                    "tags": [],
                    "timestamp": "2020-03-27T05:15:40.498316+0000",
                    "tx_id": 0,
                    "type": "eve"
                }, "_type": "_doc", "sort": [1585286141498]
            },
            "maxTs": "2020-03-27T05:15:41.498Z",
            "minTs": "2020-03-27T05:15:41.498Z",
            "escalatedCount": 0
        }],
    "duration": 14
};

let archive_alert_group = {
    "query": {
        "bool": {
            "filter": [
                {"exists": {"field": "event_type"}},
                {"term": {"event_type.keyword": "alert"}},
                {
                    "range": {
                        "@timestamp": {
                            "gte": "2020-03-29T05:15:32.626000Z",
                            "lte": "2020-03-29T05:24:20.853000Z"
                        }
                    }
                },
                {"term": {"src_ip.keyword": "10.16.1.110"}},
                {"term": {"dest_ip.keyword": "209.85.146.108"}},
                {"term": {"alert.signature_id": 2610003}}
            ],
            "must_not": [
                {"term": {"tags": "archived"}},
                {"term": {"tags": "evebox.archived"}}
            ]
        }
    },
    "script": {
        "lang": "painless",
        "inline": "\n\t\t        if (params.tags != null) {\n\t\t\t        if (ctx._source.tags == null) {\n\t\t\t            ctx._source.tags = new ArrayList();\n\t\t\t        }\n\t\t\t        for (tag in params.tags) {\n\t\t\t            if (!ctx._source.tags.contains(tag)) {\n\t\t\t                ctx._source.tags.add(tag);\n\t\t\t            }\n\t\t\t        }\n\t\t\t    }\n\t\t\t    if (ctx._source.evebox == null) {\n\t\t\t        ctx._source.evebox = new HashMap();\n\t\t\t    }\n\t\t\t    if (ctx._source.evebox.history == null) {\n\t\t\t        ctx._source.evebox.history = new ArrayList();\n\t\t\t    }\n\t\t\t    ctx._source.evebox.history.add(params.action);\n\t\t",
        "params": {
            "action": {
                "timestamp": "2020-03-29T05:27:07.647Z",
                "username": "anonymous",
                "action": "archived"
            },
            "tags": ["archived", "evebox.archived"]
        }
    }
};

let events_query = {
    "query": {
        "bool": {
            "filter": [{"exists": {"field": "event_type"}}],
            "must_not": [{"term": {"event_type": "stats"}}]
        }
    },
    "size": 500,
    "sort": [{"@timestamp": {"order": "desc"}}]
};

let events_query_for_older = {
    "query": {
        "bool": {
            "filter": [
                {"exists": {"field": "event_type"}},
                {"range": {"@timestamp": {"lte": "2020-04-01T04:18:57.889Z"}}}
            ],
            "must_not": [{"term": {"event_type": "stats"}}]
        }
    },
    "size": 500,
    "sort": [{"@timestamp": {"order": "desc"}}]
};

let events_query_for_newer = {
    "query": {
        "bool": {
            "filter": [
                {"exists": {"field": "event_type"}},
                {"range": {"@timestamp": {"gte": "2020-04-01T04:18:57.889Z"}}}
            ],
            "must_not": [{"term": {"event_type": "stats"}}]
        }
    },
    "size": 500,
    "sort": [{"@timestamp": {"order": "asc"}}]
};

let events_query_for_newer_with_search = {
    "query": {
        "bool": {
            "filter": [
                {"exists": {"field": "event_type"}},
                {
                    "query_string": {
                        "default_operator": "AND",
                        "query": "WPAD"
                    }
                },
                {"range": {"@timestamp": {"gte": "2020-03-14T17:33:41.824Z"}}}
            ],
            "must_not": [{"term": {"event_type": "stats"}}]
        }
    },
    "size": 500,
    "sort": [{"@timestamp": {"order": "asc"}}]
};

let events_query_for_oldest = {
    "query": {
        "bool": {
            "filter": [{"exists": {"field": "event_type"}}],
            "must_not": [{"term": {"event_type": "stats"}}]
        }
    }, "size": 500, "sort": [{"@timestamp": {"order": "asc"}}]
};

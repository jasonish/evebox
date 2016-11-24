package elasticsearch

import (
	"fmt"
	"github.com/jasonish/evebox/log"
	"testing"
)

func TestAggregateSum(t *testing.T) {

	log.SetLevel(log.ERROR)

	es := New("http://10.16.1.10:9200")

	size := int64(10)

	eventType := "netflow"

	query := NewEventQuery()
	query.AddFilter(TermQuery("event_type", eventType))

	aggType := "keyword"

	agg := "src_ip"

	sum := m{
		"sum": m{
			"sum": m{
				"field": "netflow.pkts",
			},
		},
	}

	if aggType == "keyword" {
		query.Aggs[agg] = map[string]interface{}{
			"terms": map[string]interface{}{
				"field": fmt.Sprintf("%s.%s", agg, es.keyword),
				"size":  size,
			},
		}
	} else {
		query.Aggs[agg] = map[string]interface{}{
			"terms": map[string]interface{}{
				"field": agg,
				"size":  size,
			},
		}
	}

	query.Aggs[agg].(map[string]interface{})["aggs"] = sum

	fmt.Println(ToJsonPretty(query))

	response, err := es.Search(query)
	if err != nil {
		t.Fatal(err)
	}

	fmt.Println(ToJsonPretty(response))

}

func Test_SqlStatement(t *testing.T) {

	sql := `SELECT
	count(source ->> 'src_ip'),
		source ->> 'src_ip',
	sum((source -> 'netflow' ->> 'pkts') :: BIGINT) AS packets
	FROM events_master
	WHERE source ->> 'event_type' = 'netflow'
GROUP BY source ->> 'src_ip',
	source ->> 'dest_ip'
ORDER BY packets DESC
	LIMIT 10;
	`

	fmt.Println(sql)
}

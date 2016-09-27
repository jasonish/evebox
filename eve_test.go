/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

package evebox

import "testing"

var tcp4EventAlert string = `{"timestamp":"2016-02-11T08:07:42.815726-0600","flow_id":140467580480416,"in_iface":"eth1","event_type":"alert","src_ip":"72.20.52.30","src_port":25565,"dest_ip":"10.16.1.236","dest_port":58686,"proto":"TCP","alert":{"action":"allowed","gid":1,"signature_id":2021701,"rev":1,"signature":"ET GAMES MINECRAFT Server response inbound","category":"Potential Corporate Privacy Violation","severity":1},"payload":"9qWANL1DeyKyeauJP7XSOBN+Wicwljhbo1b3CggiJQ7NyXWekHge","payload_printable":"...4.C{\".y..?..8.~Z'0.8[.V.\n.\"%...u..x.","stream":0,"packet":"rLwye+0ZABUXDQb3CABFGABbIh9AADQGnDhIFDQeChAB7GPd5T4vIjw6OmylI4AYAPY3ngAAAQEICiuX0AUzPUOu9qWANL1DeyKyeauJP7XSOBN+Wicwljhbo1b3CggiJQ7NyXWekHge","host":"home-firewall"}
`

func TestEvePacketToPcapTCP4(t *testing.T) {
	event, err := NewEveEventFromJson(tcp4EventAlert)
	if err != nil {
		t.Fatal(err)
	}
	pcap, err := EvePacketToPcap(event)
	if err != nil {
		t.Fatal(err)
	}
	if pcap == nil {
		t.Fatalf("Did not expect pcap to be nil.")
	}
	if len(pcap) != 145 {
		t.Fatalf("Expected len to be 145, not %d.", len(pcap))
	}
}

func TestEvePayloadToPcapTCP4(t *testing.T) {
	event, err := NewEveEventFromJson(tcp4EventAlert)
	if err != nil {
		t.Fatal(err)
	}
	pcap, err := EvePayloadToPcap(event)
	if err != nil {
		t.Fatal(err)
	}
	if pcap == nil {
		t.Fatalf("Did not expect pcap to be nil.")
	}
	if len(pcap) != 119 {
		t.Fatalf("Expected len to be 119, not %d.", len(pcap))
	}

	// file, err := os.Create("TestEvePayloadToPcapTCP4.pcap")
	// if err != nil {
	// 	log.Fatal(err)
	// }
	// file.Write(pcap)
	// file.Close()

}

var icmp4EveAlert string = `{"timestamp":"2016-02-09T03:14:52.232074-0600","flow_id":140467580337840,"in_iface":"eth1","event_type":"alert","src_ip":"10.16.1.1","dest_ip":"10.16.1.5","proto":"ICMP","icmp_type":3,"icmp_code":3,"alert":{"action":"allowed","gid":1,"signature_id":2100402,"rev":8,"signature":"GPL ICMP_INFO Destination Unreachable Port Unreachable","category":"Misc activity","severity":3},"payload":"RQAATgAAQABAESR6ChABBQoQAQGHaQCJADotAPYfAAAAAQAAAAAAACBDS0FBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQQAAIQAK","payload_printable":"E..N..@.@.$z\n...\n....i...:-............. CKAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA..!.\n","stream":0,"packet":"xBL1ZkOIABUXDQb3CABFwABqvNYAAEABptcKEAEBChABBQMDE24AAAAARQAATgAAQABAESR6ChABBQoQAQGHaQCJADotAPYfAAAAAQAAAAAAACBDS0FBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQQAAIQAK","host":"home-firewall"}`

func TestEvePacketToPcap(t *testing.T) {
	event, err := NewEveEventFromJson(icmp4EveAlert)
	if err != nil {
		t.Fatal(err)
	}
	pcap, err := EvePacketToPcap(event)
	if err != nil {
		t.Fatal(err)
	}
	if pcap == nil {
		t.Fatalf("Did not expect pcap to be nil.")
	}
	if len(pcap) != 160 {
		t.Fatalf("Expected pcap length to be 160, not %d.", len(pcap))
	}
}

func TestEvePayloadToPcap(t *testing.T) {
	event, err := NewEveEventFromJson(icmp4EveAlert)
	if err != nil {
		t.Fatal(err)
	}
	pcap, err := EvePayloadToPcap(event)
	if err != nil {
		t.Fatal(err)
	}
	if pcap == nil {
		t.Fatalf("Did not expect pcap to be nil.")
	}
	if len(pcap) != 146 {
		t.Fatalf("Expected pcap length to be 146, not %d.", len(pcap))
	}

	// file, err := os.Create("test3.pcap")
	// if err != nil {
	// 	log.Fatal(err)
	// }
	// file.Write(pcap)
	// file.Close()

}

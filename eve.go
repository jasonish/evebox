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

package main

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"net"
	"strconv"
	"strings"
	"time"

	"github.com/google/gopacket"
	"github.com/google/gopacket/layers"
)

const (
	// The even timestamp format.
	RFC3339Nano_Modified = "2006-01-02T15:04:05.999999999Z0700"
)

// Eve Timestamp.
type EveTimestamp struct {
	time.Time
}

func (ev *EveTimestamp) UnmarshalJSON(b []byte) (err error) {
	// Trim edge quotes.
	if b[0] == '"' && b[len(b)-1] == '"' {
		b = b[1 : len(b)-1]
	}

	ev.Time, err = time.Parse(RFC3339Nano_Modified, string(b))
	return err
}

// Eve Buffer
//
// In the EVE JSON, some fields such as packet and payload are base64
// encoded. This will allow us to decode the base64 as part of the
// unmarshalling process.
type EveBuffer []byte

func (eb *EveBuffer) UnmarshalJSON(b []byte) (err error) {
	// Trim edge quotes.
	if b[0] == '"' && b[len(b)-1] == '"' {
		b = b[1 : len(b)-1]
	}

	*eb, err = base64.StdEncoding.DecodeString(string(b))
	return err
}

func (eb *EveBuffer) Bytes() []byte {
	return *eb
}

type EveEvent struct {
	Timestamp EveTimestamp `json:"timestamp"`
	SrcIP     string       `json:"src_ip"`
	DstIP     string       `json:"dest_ip"`
	Proto     string       `json:"proto"`
	SrcPort   uint16       `json:"src_port"`
	DstPort   uint16       `json:"dest_port"`
	IcmpType  uint8        `json:"icmp_type"`
	IcmpCode  uint8        `json:"icmp_code"`
	Payload   EveBuffer    `json:"payload"`
	Packet    EveBuffer    `json:"packet"`
	Buffer    EveBuffer    `json:"buffer"`
}

// Create a new EveEvent from a JSON string.
func NewEveEventFromJson(eveJson string) (event *EveEvent, err error) {
	bytes := []byte(eveJson)
	err = json.Unmarshal(bytes, &event)
	if err != nil {
		return nil, err
	}
	return event, err
}

// Given a protocol name as a string (could be a number), return the
// IPProtocol for that protocol.
func ProtoNumber(proto string) (layers.IPProtocol, error) {

	switch strings.ToLower(proto) {
	case "tcp":
		return layers.IPProtocolTCP, nil
	case "udp":
		return layers.IPProtocolUDP, nil
	case "icmp":
		return layers.IPProtocolICMPv4, nil
	case "ipv6-icmp":
		return layers.IPProtocolICMPv6, nil
	}

	// Is the proto a number already?
	if val, err := strconv.Atoi(proto); err == nil {
		return layers.IPProtocol(val), nil
	}

	return 0, errors.New("unknown protocol")
}

// Convert the packet of an EveEvent to a PCAP file. A buffer
// representing a complete PCAP file is returned.
func EvePacketToPcap(event *EveEvent) ([]byte, error) {
	return CreatePcap(event.Timestamp.Time,
		event.Packet, layers.LinkTypeEthernet)
}

// Given an EvePacket, convert the payload to a PCAP faking out the
// headers as best we can.
//
// A buffer containing the 1 packet pcap file will be returned.
func EvePayloadToPcap(event *EveEvent) ([]byte, error) {
	buffer := gopacket.NewSerializeBuffer()
	options := gopacket.SerializeOptions{
		FixLengths:       true,
		ComputeChecksums: true,
	}

	payloadLayer := gopacket.Payload(event.Payload.Bytes())
	payloadLayer.SerializeTo(buffer, options)

	srcIp := net.ParseIP(event.SrcIP)
	if srcIp == nil {
		return nil, fmt.Errorf("Failed to parse IP address %s.", event.SrcIP)
	}
	dstIp := net.ParseIP(event.DstIP)
	if dstIp == nil {
		return nil, fmt.Errorf("Failed to parse IP address %s.", event.DstIP)
	}

	proto, err := ProtoNumber(event.Proto)
	if err != nil {
		return nil, err
	}

	switch proto {
	case layers.IPProtocolTCP:
		// Could probably fake up a better TCP layer here.
		tcpLayer := layers.TCP{
			SrcPort: layers.TCPPort(event.SrcPort),
			DstPort: layers.TCPPort(event.DstPort),
		}
		tcpLayer.SerializeTo(buffer, options)
		break
	case layers.IPProtocolUDP:
		udpLayer := layers.UDP{
			SrcPort: layers.UDPPort(event.SrcPort),
			DstPort: layers.UDPPort(event.DstPort),
		}
		udpLayer.SerializeTo(buffer, options)
		break
	case layers.IPProtocolICMPv4:
		icmpLayer := layers.ICMPv4{
			TypeCode: layers.CreateICMPv4TypeCode(
				event.IcmpType, event.IcmpCode),
			Id:  0,
			Seq: 0,
		}
		icmpLayer.SerializeTo(buffer, options)
		break
	case layers.IPProtocolICMPv6:
		icmp6Layer := layers.ICMPv6{
			TypeCode: layers.CreateICMPv6TypeCode(
				event.IcmpType, event.IcmpCode),
		}
		icmp6Layer.SerializeTo(buffer, options)
		break
	default:
		return nil, fmt.Errorf("Unsupported protocol %d.", proto)
	}

	isIp6 := dstIp.To4() == nil

	if !isIp6 {
		ipLayer := layers.IPv4{
			SrcIP:    srcIp,
			DstIP:    dstIp,
			Version:  4,
			Protocol: proto,
			TTL:      64,
		}
		ipLayer.SerializeTo(buffer, options)
	} else {
		ip6Layer := layers.IPv6{
			Version: 6,
			SrcIP:   srcIp,
			DstIP:   dstIp,
		}
		ip6Layer.SerializeTo(buffer, options)
	}

	return CreatePcap(event.Timestamp.Time,
		buffer.Bytes(), layers.LinkTypeRaw)
}

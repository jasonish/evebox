/* Copyright (c) 2017 Jason Ish
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

// Based on generate_cert.go found in the crypto/tls package.

package gencert

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"github.com/jasonish/evebox/log"
	"github.com/spf13/pflag"
	"math/big"
	"net"
	"os"
	"strings"
	"time"
)

const RSA_BITS = 2048

func Main(args []string) {

	var hostname string
	var org string
	var duration int
	var outputFilename string

	flagset := pflag.NewFlagSet("gencert", 0)
	flagset.StringVar(&hostname, "hostname", "",
		"Hostname or IP address (one or more, comma separated)")
	flagset.StringVar(&org, "org", "EveBox User", "Organization name")
	flagset.IntVar(&duration, "duration", 365,
		"Duration that certificate is valid for in days")
	flagset.StringVarP(&outputFilename, "outputFilename", "o", "",
		"Output file (eg. evebox.pem)")
	if err := flagset.Parse(args); err != nil {
		if err == pflag.ErrHelp {
			os.Exit(0)
		}
		os.Exit(1)
	}

	key, err := rsa.GenerateKey(rand.Reader, RSA_BITS)
	if err != nil {
		log.Fatalf("Failed to generate private key: %v", err)
	}

	notBefore := time.Now()
	notAfter := notBefore.AddDate(0, 0, duration)

	log.Printf("Orgnanization: %s", org)
	log.Printf("Hostnames: %s", hostname)
	log.Printf("Key type: RSA")
	log.Printf("Key bits: %d", RSA_BITS)
	log.Printf("Valid not before: %v", notBefore)
	log.Printf("Valid not after: %v", notAfter)

	serialNumberLimit := new(big.Int).Lsh(big.NewInt(1), 128)
	serialNumber, err := rand.Int(rand.Reader, serialNumberLimit)
	if err != nil {
		log.Fatalf("Failed to generate serial number: %v", err)
	}
	template := x509.Certificate{
		SerialNumber: serialNumber,
		Subject: pkix.Name{
			Organization: []string{org},
		},
		NotBefore:             notBefore,
		NotAfter:              notAfter,
		KeyUsage:              x509.KeyUsageKeyEncipherment | x509.KeyUsageDigitalSignature,
		ExtKeyUsage:           []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		BasicConstraintsValid: true,
	}

	hosts := strings.Split(hostname, ",")
	for _, host := range hosts {
		if addr := net.ParseIP(host); addr != nil {
			log.Info("Adding IP address %s", host)
			template.IPAddresses = append(template.IPAddresses, addr)
		} else {
			log.Info("Adding hostname %s.", host)
			template.DNSNames = append(template.DNSNames, host)
		}
	}

	derBytes, err := x509.CreateCertificate(rand.Reader, &template, &template,
		&key.PublicKey, key)
	if err != nil {
		log.Fatalf("Failed to create certificate: %v", err)
	}

	output := os.Stdout

	if outputFilename != "" {
		output, err = os.Create(outputFilename)
		if err != nil {
			log.Fatal("Failed to open %s for writing: %v",
				outputFilename, err)
		}
		defer output.Close()
	}

	pem.Encode(output, &pem.Block{
		Type: "CERTIFICATE", Bytes: derBytes,
	})
	pem.Encode(output, &pem.Block{
		Type:  "RSA PRIVATE KEY",
		Bytes: x509.MarshalPKCS1PrivateKey(key),
	})
}

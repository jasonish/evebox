package evebox

type ArchiveService interface {
	ArchiveAlerts(signatureId uint64,
		srcIp string, destIp string,
		minTimestamp string, maxTimestamp string) error
}

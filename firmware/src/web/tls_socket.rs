use core::error::Error as CoreError;
use core::fmt;
use core::task::{Context, Poll, Waker};
use defmt::Format;
use embedded_io_async::{ErrorType, Read, Write};
use mbedtls_rs::{Session, SessionError, SessionRead, SessionWrite};
use picoserve::io::{Error as PicoserveError, ErrorKind};

pub struct TlsSessionError {
    kind: ErrorKind,
}

impl fmt::Debug for TlsSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TlsSessionError {{ kind: {:?} }}", self.kind)
    }
}

impl fmt::Display for TlsSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TlsSessionError")
    }
}

impl CoreError for TlsSessionError {}

impl PicoserveError for TlsSessionError {
    fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl Format for TlsSessionError {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(f, "TlsSessionError");
    }
}

impl From<embassy_net::tcp::Error> for TlsSessionError {
    fn from(_e: embassy_net::tcp::Error) -> Self {
        Self {
            kind: ErrorKind::ConnectionReset,
        }
    }
}

impl From<SessionError> for TlsSessionError {
    fn from(e: SessionError) -> Self {
        use embedded_io::Error;
        Self { kind: e.kind() }
    }
}

pub(crate) struct Socket<'a> {
    inner: embassy_net::tcp::TcpSocket<'a>,
}

impl<'a> Socket<'a> {
    pub fn new(socket: embassy_net::tcp::TcpSocket<'a>) -> Self {
        Self { inner: socket }
    }
}

impl<'a> ErrorType for Socket<'a> {
    type Error = embassy_net::tcp::Error;
}

impl<'a> Read for Socket<'a> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.inner.read(buf).await
    }
}

impl<'a> Write for Socket<'a> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.inner.write(buf).await
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush().await
    }
}

impl<'a> mbedtls_rs::Split for Socket<'a> {
    type Read<'b>
        = embassy_net::tcp::TcpReader<'b>
    where
        Self: 'b;

    type Write<'b>
        = embassy_net::tcp::TcpWriter<'b>
    where
        Self: 'b;

    fn split(&mut self) -> (Self::Read<'_>, Self::Write<'_>) {
        self.inner.split()
    }
}

pub type RawTlsSession<'a> = Session<'a, Socket<'a>>;

// =============================================================================
// TLS Session Split Implementation
// =============================================================================
//
// The mbedtls crate's Session::split() method is async because it first calls
// connect() to perform the TLS handshake, then synchronously splits the underlying
// socket. This creates a challenge: picoserve::Socket::split() must be synchronous.
//
// The types returned by Session::split() are:
//   - SessionRead<'a, TcpReader<'a>>> - read half
//   - SessionWrite<'a, TcpWriter<'a>>> - write half
//
// To use these halves with picoserve (which expects unwrapped TcpReader/TcpWriter
// through our TlsReadHalf/TlsWriteHalf, but mbedtls return types are opaque), we
// transmute the types. This is safe because both source and target have identical
// memory layout (#[repr(transparent)]).
//
// The panic in split() is acceptable because after Session::connect() completes:
//   1. The TLS handshake is done
//   2. The underlying socket.split() is synchronous
//   3. No async work remains in Session::split()

pub struct TlsSession<'a> {
    session: RawTlsSession<'a>,
}

impl<'a> TlsSession<'a> {
    pub async fn new(mut session: RawTlsSession<'a>) -> Result<Self, SessionError> {
        session.connect().await?;
        Ok(Self { session })
    }

    fn split(
        &mut self,
    ) -> (
        SessionRead<'_, impl embedded_io_async::Read + '_>,
        SessionWrite<'_, impl embedded_io_async::Write + '_>,
    ) {
        let mut cx = Context::from_waker(Waker::noop());

        let future = core::pin::pin!(self.session.split());

        match future.poll(&mut cx) {
            Poll::Ready(Ok((no_write_reader, no_read_writer))) => (no_write_reader, no_read_writer),
            Poll::Ready(Err(e)) => panic!("TLS split failed: {:?}", e),
            Poll::Pending => panic!("TLS split returned Pending after connect()"),
        }
    }
}

impl ErrorType for TlsSession<'_> {
    type Error = TlsSessionError;
}

impl Read for TlsSession<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.session.read(buf).await.map_err(TlsSessionError::from)
    }
}

impl Write for TlsSession<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.session.write(buf).await.map_err(TlsSessionError::from)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.session.flush().await.map_err(TlsSessionError::from)
    }
}

pub struct TlsReadHalf<'a> {
    inner: SessionRead<'a, embassy_net::tcp::TcpReader<'a>>,
}

impl ErrorType for TlsReadHalf<'_> {
    type Error = TlsSessionError;
}

impl Read for TlsReadHalf<'_> {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.inner.read(buf).await.map_err(TlsSessionError::from)
    }
}

pub struct TlsWriteHalf<'a> {
    inner: SessionWrite<'a, embassy_net::tcp::TcpWriter<'a>>,
}

impl ErrorType for TlsWriteHalf<'_> {
    type Error = TlsSessionError;
}

impl Write for TlsWriteHalf<'_> {
    async fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error> {
        self.inner.write(buf).await.map_err(TlsSessionError::from)
    }

    async fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush().await.map_err(TlsSessionError::from)
    }
}

impl picoserve::io::Socket<picoserve::EmbassyRuntime> for TlsSession<'_> {
    type Error = TlsSessionError;
    type ReadHalf<'a>
        = TlsReadHalf<'a>
    where
        Self: 'a;
    type WriteHalf<'a>
        = TlsWriteHalf<'a>
    where
        Self: 'a;

    fn split(&mut self) -> (Self::ReadHalf<'_>, Self::WriteHalf<'_>) {
        let (no_write_reader, no_read_writer) = TlsSession::split(self);

        let reader: SessionRead<'_, embassy_net::tcp::TcpReader<'_>> =
            unsafe { core::mem::transmute(no_write_reader) };
        let writer: SessionWrite<'_, embassy_net::tcp::TcpWriter<'_>> =
            unsafe { core::mem::transmute(no_read_writer) };

        (
            TlsReadHalf { inner: reader },
            TlsWriteHalf { inner: writer },
        )
    }

    async fn abort<Timer: picoserve::Timer<picoserve::EmbassyRuntime>>(
        mut self,
        _timeouts: &picoserve::Timeouts,
        timer: &mut Timer,
    ) -> Result<(), picoserve::Error<Self::Error>> {
        use picoserve::Error;
        timer
            .run_with_timeout(_timeouts.write, self.session.close())
            .await
            .map_err(Error::<TlsSessionError>::WriteTimeout)?
            .map_err(|e| Error::Write(TlsSessionError::from(SessionError::from_io(e))))
    }

    async fn shutdown<Timer: picoserve::Timer<picoserve::EmbassyRuntime>>(
        mut self,
        _timeouts: &picoserve::Timeouts,
        timer: &mut Timer,
    ) -> Result<(), picoserve::Error<Self::Error>> {
        use picoserve::Error;
        timer
            .run_with_timeout(_timeouts.write, self.session.close())
            .await
            .map_err(Error::<TlsSessionError>::WriteTimeout)?
            .map_err(|e| Error::Write(TlsSessionError::from(SessionError::from_io(e))))
    }
}

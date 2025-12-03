use core::net::Ipv4Addr;
use edge_mdns::buf::{BufferAccess, VecBufAccess};
use edge_mdns::domain::base::Ttl;
use edge_mdns::io::{self, DEFAULT_SOCKET, MdnsIoError};
use edge_mdns::{HostAnswersMdnsHandler, host::Host};
use edge_nal::UdpSplit;
use edge_nal_embassy::{Udp, UdpBuffers};
use embassy_net::Stack;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use esp_println::println;

const SERVICE_NAME: &str = "phoniesp32";

#[embassy_executor::task]
pub async fn mdns_responder(stack: Stack<'static>) {
    println!("Starting mDNS responder for {}", SERVICE_NAME);

    // Wait for network to be ready and get IP
    let ipv4 = wait_for_network_ready(&stack).await;

    let (recv_buf, send_buf) = mk_static!(
        (
            VecBufAccess<NoopRawMutex, 1500>,
            VecBufAccess<NoopRawMutex, 1500>
        ),
        (VecBufAccess::new(), VecBufAccess::new(),)
    );

    let udp_buffers = mk_static!(UdpBuffers<1>, UdpBuffers::new());
    let udp = Udp::new(stack, udp_buffers);

    let signal = Signal::new();

    loop {
        match run_mdns(udp, &*recv_buf, &*send_buf, &signal, ipv4).await {
            Ok(_) => {
                println!("mDNS responder completed successfully");
                Timer::after(Duration::from_secs(1)).await;
            }
            Err(e) => {
                println!("mDNS responder error: {:?}", e);
                Timer::after(Duration::from_secs(5)).await;
            }
        }
    }
}

async fn wait_for_network_ready(stack: &Stack<'_>) -> Ipv4Addr {
    loop {
        if let Some(config) = stack.config_v4() {
            let ip = config.address.address();
            if !ip.is_unspecified() {
                println!("Network ready, IP: {}", ip);
                return ip;
            }
        }
        Timer::after(Duration::from_millis(500)).await;
    }
}

async fn run_mdns<RB, SB>(
    udp: Udp<'_>,
    recv_buf: RB,
    send_buf: SB,
    signal: &Signal<NoopRawMutex, ()>,
    ipv4: Ipv4Addr,
) -> Result<(), MdnsIoError<edge_nal_embassy::UdpError>>
where
    RB: BufferAccess<[u8]>,
    SB: BufferAccess<[u8]>,
{
    let mut socket = io::bind(&udp, DEFAULT_SOCKET, Some(Ipv4Addr::UNSPECIFIED), Some(0)).await?;

    let (recv, send) = socket.split();

    let host = Host {
        hostname: SERVICE_NAME,
        ipv4,
        ipv6: core::net::Ipv6Addr::UNSPECIFIED,
        ttl: Ttl::from_secs(60),
    };

    let mdns = io::Mdns::<NoopRawMutex, _, _, _, _>::new(
        Some(Ipv4Addr::UNSPECIFIED),
        Some(0),
        recv,
        send,
        recv_buf,
        send_buf,
        |buf| {
            // Simple random number generator for mDNS
            use core::sync::atomic::{AtomicU32, Ordering};
            static SEED: AtomicU32 = AtomicU32::new(12345);
            let seed = SEED.fetch_add(1, Ordering::Relaxed);
            let mut x = seed;
            for byte in buf.iter_mut() {
                x = x.wrapping_mul(1103515245).wrapping_add(12345);
                *byte = (x >> 16) as u8;
            }
        },
        signal,
    );

    mdns.run(HostAnswersMdnsHandler::new(&host)).await
}


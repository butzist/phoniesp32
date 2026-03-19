use core::net::{Ipv4Addr, SocketAddr};

use edge_dhcp::io::DEFAULT_SERVER_PORT;
use edge_nal::UdpBind;
use edge_nal_embassy::{Udp, UdpBuffers};
use embassy_executor::Spawner;
use embassy_net::Stack;
use embassy_time::Timer;
use esp_println::println;

const AP_IP: Ipv4Addr = Ipv4Addr::new(192, 168, 42, 1);
const CAPTIVE_DNS_PORT: u16 = 53;

pub struct CaptivePortal {
    stack: Stack<'static>,
}

impl CaptivePortal {
    pub fn new(stack: Stack<'static>) -> Self {
        Self { stack }
    }

    pub fn spawn(self, spawner: &Spawner) {
        spawner.must_spawn(dhcp_task(self.stack));
        spawner.must_spawn(captive_dns_task(self.stack));
    }
}

#[embassy_executor::task]
async fn dhcp_task(stack: Stack<'static>) {
    println!("Starting DHCP server");

    let mut server = edge_dhcp::server::Server::<_, 8>::new_with_et(AP_IP);

    let mut gw_buf = [Ipv4Addr::UNSPECIFIED; 1];
    let mut server_options = edge_dhcp::server::ServerOptions::new(AP_IP, Some(&mut gw_buf));
    server_options.dns = &[AP_IP];
    server_options.captive_url = Some("http://phoniesp32.local");

    let udp_buffers = mk_static!(UdpBuffers<1, 576, 576>, UdpBuffers::new());
    let udp = Udp::new(stack, udp_buffers);

    let mut buf = [0u8; 576];

    loop {
        wait_for_network(&stack).await;

        let socket_addr = SocketAddr::new(
            core::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            DEFAULT_SERVER_PORT,
        );

        match udp.bind(socket_addr).await {
            Ok(mut socket) => {
                println!("DHCP server listening on port {}", DEFAULT_SERVER_PORT);
                if let Err(e) =
                    edge_dhcp::io::server::run(&mut server, &server_options, &mut socket, &mut buf)
                        .await
                {
                    println!("DHCP server error: {:?}", e);
                }
            }
            Err(e) => {
                println!("DHCP bind error: {:?}", e);
            }
        }

        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}

#[embassy_executor::task]
async fn captive_dns_task(stack: Stack<'static>) {
    println!("Starting captive DNS server");

    let udp_buffers = mk_static!(UdpBuffers<1>, UdpBuffers::new());
    let udp = Udp::new(stack, udp_buffers);

    let mut tx_buf = [0u8; 1024];
    let mut rx_buf = [0u8; 1024];

    loop {
        wait_for_network(&stack).await;

        let socket_addr = SocketAddr::new(
            core::net::IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            CAPTIVE_DNS_PORT,
        );

        match edge_captive::io::run(
            &udp,
            socket_addr,
            &mut tx_buf,
            &mut rx_buf,
            AP_IP,
            core::time::Duration::from_secs(60),
        )
        .await
        {
            Ok(_) => {
                println!("Captive DNS completed");
            }
            Err(e) => {
                println!("Captive DNS error: {:?}", e);
            }
        }

        Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}

async fn wait_for_network(stack: &Stack<'_>) {
    loop {
        if stack.is_link_up()
            && let Some(config) = stack.config_v4()
            && !config.address.address().is_unspecified()
        {
            return;
        }
        Timer::after(embassy_time::Duration::from_millis(500)).await;
    }
}

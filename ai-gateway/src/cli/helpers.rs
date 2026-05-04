use std::{fmt::Write as _, net::SocketAddr};

use crate::config::Config;

pub fn show_welcome_banner(addr: &SocketAddr, config: &Config) {
    print_ascii_banner();
    print_example_curl(addr);

    if let Some(router_config) = autodefault_router(config) {
        print_autodefault_section(addr, router_config);
    }
}

fn print_ascii_banner() {
    let banner = format!(
        "{}{}{}",
        "\x1b[36m",
        r"
               -*******-               
          :-***--------****=           
      --***-----------------***--      
   ****-------------------------****  
   ******--------------------*****=*             _    ___       ____    _  _____ _______        ___ __   __
   *-----****-------------*****-   *            / \  |_ _|     / ___|  / \|_   _| ____\ \      / / \\ \ / /
   *---------*****---*****--**    **           / _ \  | |_____| |  _  / _ \ | | |  _|  \ \ /\ / / _ \\ V / 
   *--------**=   -**------*=    *=*          / ___ \ | |_____| |_| |/ ___ \| | | |___  \ V  V / ___ \| |  
   *-----**-     =*------**=   -*  *         /_/   \_\___|     \____/_/   \_\_| |_____|  \_/\_/_/   \_\_|  
   *---**      **  *----**     *   *  
   ***=     -*=    *---**    *     *                            By Helicone.ai
   *      *        *--*     *-     * 
   *   **          ***     *-      *                             
   ***=            **     *      -**  
      =**--        *:   *  --**=    
          :***-    *   *****          
              --*******--",
        "\x1b[0m"
    );

    println!(
        "{banner}\n\n\x1b[1m🚀 Welcome to AI Gateway! \x1b[0m\n"
    );
}

fn print_example_curl(addr: &SocketAddr) {
    println!(
        "Try it out with this example request:\n\n\
         \x1b[0mcurl --request POST \\\n\
         \x20 --url http://{addr:?}/ai/chat/completions \\\n\
         \x20 --header 'Content-Type: application/json' \\\n\
         \x20 --data '{{\n\
         \x20   \"model\": \"openai/gpt-5-mini\",\n\
         \x20   \"messages\": [\n\
         \x20     {{\n\
         \x20       \"role\": \"user\",\n\
         \x20       \"content\": \"hello world\"\n\
         \x20     }}\n\
         \x20   ]\n\
         \x20 }}'\x1b[0m"
    );
}

fn autodefault_router(
    config: &Config,
) -> Option<&crate::config::router::RouterConfig> {
    if !config.has_autodefault_router() {
        return None;
    }
    config.routers.get(&Config::autodefault_router_id())
}

fn print_autodefault_section(
    addr: &SocketAddr,
    router_config: &crate::config::router::RouterConfig,
) {
    let mut section = String::from(
        "\n\n\x1b[1mAutodefault router\x1b[0m  \
         /router/autodefault/chat/completions\n",
    );

    print_strategy_and_providers(&mut section, router_config);
    print_autodefault_curl(&mut section, addr);

    print!("{section}");
}

fn print_strategy_and_providers(
    out: &mut String,
    router_config: &crate::config::router::RouterConfig,
) {
    use crate::endpoints::EndpointType;

    let Some(balance) = router_config.load_balance.as_ref().get(&EndpointType::Chat) else {
        return;
    };

    let strategy_name = balance.as_ref();
    let providers = balance.providers();

    write!(out, "  Strategy : \x1b[33m{strategy_name}\x1b[0m\n")
        .expect("write to String");
    write!(out, "  Providers: ").expect("write to String");

    for (i, provider) in providers.iter().enumerate() {
        if i > 0 {
            write!(out, ", ").expect("write to String");
        }
        write!(out, "\x1b[32m{provider}\x1b[0m").expect("write to String");
    }
    writeln!(out).expect("write to String");
}

fn print_autodefault_curl(out: &mut String, addr: &SocketAddr) {
    write!(
        out,
        "\n\x1b[0mcurl --request POST \\\n\
         \x20 --url http://{addr:?}/router/autodefault/chat/completions \\\n\
         \x20 --header 'Content-Type: application/json' \\\n\
         \x20 --data '{{\n\
         \x20   \"model\": \"openai/gpt-5-mini\",\n\
         \x20   \"messages\": [\n\
         \x20     {{\n\
         \x20       \"role\": \"user\",\n\
         \x20       \"content\": \"hello world\"\n\
         \x20     }}\n\
         \x20   ]\n\
         \x20 }}'\x1b[0m\n"
    )
    .expect("write to String");
}

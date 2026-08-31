use oracledb::{Pool, PoolConfig};

pub fn create_oracle_pool(
    host: &str,
    port: &str,
    service_name: &str,
    user: &str,
    password: &str,
) -> Result<Pool, String> {
    let connect_string = format!("{}:{}/{}", host, port, service_name);
    let pool_config = PoolConfig::default()
        .set_connect_string(&connect_string)
        .map_err(|e| format!("OracleDB config error: {:?}", e))?
        .set_credentials(user, password);

    oracledb::create_pool(pool_config).map_err(|e| format!("Pool creation error: {:?}", e))
}

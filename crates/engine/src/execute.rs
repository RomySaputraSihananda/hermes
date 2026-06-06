pub(crate) fn build_trade_request(
    symbol: &str,
    signal: &ict::TradeSignal,
    volume: rust_decimal::Decimal,
) -> mt5_client::TradeRequest {
    let order_type: u32 = match signal.side {
        domain::Side::Long  => 0, // ORDER_TYPE_BUY
        domain::Side::Short => 1, // ORDER_TYPE_SELL
    };
    mt5_client::TradeRequest {
        action:     1, // TRADE_ACTION_DEAL
        symbol:     symbol.to_string(),
        volume:     volume.to_string().parse::<f64>().unwrap_or(0.0),
        order_type,
        price:      signal.entry.to_string().parse::<f64>().unwrap_or(0.0),
        sl:         Some(signal.sl.to_string().parse::<f64>().unwrap_or(0.0)),
        tp:         Some(signal.tp.to_string().parse::<f64>().unwrap_or(0.0)),
        magic:      None,
        comment:    None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::Side;
    use ict::{ConfluenceFlags, TradeSignal};

    fn make_signal(side: Side, entry: &str, sl: &str, tp: &str) -> TradeSignal {
        TradeSignal {
            side,
            entry: entry.parse().unwrap(),
            sl:    sl.parse().unwrap(),
            tp:    tp.parse().unwrap(),
            confluence: ConfluenceFlags::default(),
        }
    }

    #[test]
    fn build_trade_request_buy() {
        let signal = make_signal(Side::Long, "1.1000", "1.0950", "1.1100");
        let req = build_trade_request("EURUSD", &signal, "0.10".parse().unwrap());
        assert_eq!(req.action, 1);        // TRADE_ACTION_DEAL
        assert_eq!(req.order_type, 0);    // ORDER_TYPE_BUY
        assert_eq!(req.symbol, "EURUSD");
        assert!((req.price   - 1.1000_f64).abs() < 1e-6);
        assert!((req.volume  - 0.10_f64 ).abs() < 1e-6);
        assert!(req.sl.is_some());
        assert!(req.tp.is_some());
        assert!(req.magic.is_none());
        assert!(req.comment.is_none());
    }

    #[test]
    fn build_trade_request_sell() {
        let signal = make_signal(Side::Short, "1.1000", "1.1050", "1.0900");
        let req = build_trade_request("EURUSD", &signal, "0.05".parse().unwrap());
        assert_eq!(req.action, 1);
        assert_eq!(req.order_type, 1);    // ORDER_TYPE_SELL
    }
}

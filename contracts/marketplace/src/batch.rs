use soroban_sdk::{Address, Env, Vec, token};
use crate::errors::Error;
use crate::storage::*;
use crate::types::*;
use crate::events::*;

pub fn batch_add_product(
    e: &Env,
    seller: Address,
    products: Vec<BatchCreateProductInput>,
) -> Result<Vec<u64>, Error> {
    seller.require_auth();
    
    let config = get_config(e).ok_or(Error::NotInitialized)?;
    if config.is_paused {
        return Err(Error::MarketplacePaused);
    }

    // Verify seller exists and is verified
    let seller_data = get_seller(e, &seller).ok_or(Error::SellerNotFound)?;
    if seller_data.status != SellerStatus::Verified {
        return Err(Error::SellerNotVerified);
    }
    if seller_data.status == SellerStatus::Suspended {
        return Err(Error::SellerSuspended);
    }

    let mut product_ids = Vec::new(e);

    for input in products.iter() {
        if input.name.len() == 0 || input.description.len() == 0 {
            return Err(Error::InvalidMetadata);
        }
        if input.price == 0 || input.stock_quantity == 0 {
            return Err(Error::InvalidInput);
        }

        // Verify category
        if !category_exists(e, input.category_id) {
            return Err(Error::CategoryNotFound);
        }

        let product_id = get_next_product_id(e);

        let product = Product {
            id: product_id,
            seller: seller.clone(),
            name: input.name,
            description: input.description,
            category_id: input.category_id,
            price: input.price,
            status: ProductStatus::Active,
            stock_quantity: input.stock_quantity,
            rating: 0,
            purchase_count: 0,
            created_at: e.ledger().timestamp(),
            metadata: input.metadata,
        };

        set_product(e, &product);
        add_seller_product(e, &seller, product_id);
        add_category_product(e, input.category_id, product_id);
        increment_product_counter(e);
        
        product_ids.push_back(product_id);

        ProductListedEventData {
            seller: seller.clone(),
        }
        .publish(e);
    }

    let mut updated_config = config;
    updated_config.total_products += products.len() as u64;
    updated_config.updated_at = e.ledger().timestamp();
    set_config(e, &updated_config);

    extend_instance_ttl(e);
    Ok(product_ids)
}

pub fn batch_create_order(
    e: &Env,
    buyer: Address,
    token: Address,
    orders: Vec<BatchCreateOrderInput>,
) -> Result<Vec<u64>, Error> {
    buyer.require_auth();

    let config = get_config(e).ok_or(Error::NotInitialized)?;
    if config.is_paused {
        return Err(Error::MarketplacePaused);
    }

    let mut total_amount: u128 = 0;
    let mut order_ids = Vec::new(e);
    let mut product_updates: Vec<(u64, Product)> = Vec::new(e);

    // First pass: Validate and calculate total
    for input in orders.iter() {
        let mut product = get_product(e, input.product_id).ok_or(Error::ProductNotFound)?;
        
        if product.status != ProductStatus::Active {
            return Err(Error::InvalidProductStatus);
        }
        if product.stock_quantity < input.quantity {
            return Err(Error::InsufficientStock);
        }

        let line_total = product.price.checked_mul(input.quantity as u128).ok_or(Error::FeeOverflow)?;
        total_amount = total_amount.checked_add(line_total).ok_or(Error::FeeOverflow)?;
        
        // Decrement stock in memory
        product.stock_quantity -= input.quantity;
        product.purchase_count += 1;
        if product.stock_quantity == 0 {
            product.status = ProductStatus::OutOfStock;
        }
        product_updates.push_back((input.product_id, product));
    }

    // Transfer tokens to contract (Escrow)
    if total_amount > 0 {
        let token_client = token::Client::new(e, &token);
        token_client.transfer(&buyer, &e.current_contract_address(), &(total_amount as i128));
    }

    // Second pass: Commit changes
    for i in 0..orders.len() {
        let input = orders.get(i).unwrap();
        let (_, product) = product_updates.get(i).unwrap(); // Corresponding product update
        
        // Save product with new stock
        set_product(e, &product);

        // Create Order
        let order_id = get_next_order_id(e);
        let order_total = product.price * (input.quantity as u128);

        let order = Order {
            id: order_id,
            buyer: buyer.clone(),
            seller: product.seller.clone(),
            product_id: input.product_id,
            quantity: input.quantity,
            total_price: order_total,
            status: OrderStatus::Paid, // Automatically paid via batch transfer
            created_at: e.ledger().timestamp(),
            updated_at: e.ledger().timestamp(),
            escrow_balance: order_total,
        };

        set_order(e, &order);
        add_buyer_order(e, &buyer, order_id);
        add_seller_order(e, &product.seller, order_id);
        increment_order_counter(e);
        
        order_ids.push_back(order_id);
    }
    
    extend_instance_ttl(e);
    Ok(order_ids)
}

pub fn batch_update_order_status(
    e: &Env,
    caller: Address,
    updates: Vec<BatchUpdateStatusInput>,
) -> Result<(), Error> {
    caller.require_auth();

    for input in updates.iter() {
        let mut order = get_order(e, input.order_id).ok_or(Error::OrderNotFound)?;
        let new_status = OrderStatus::from_u32(input.new_status).ok_or(Error::InvalidInput)?;

        // Authorization logic
        // Seller can mark as Shipped
        // Buyer can mark as Delivered (and potentially release escrow)
        // Admin/Arbiter can Resolve Dispute (not implemented here yet)
        
        if caller == order.seller {
             match new_status {
                OrderStatus::Shipped => {
                    if order.status != OrderStatus::Paid {
                        return Err(Error::InvalidOrderStatus);
                    }
                    order.status = OrderStatus::Shipped;
                },
                OrderStatus::Cancelled => {
                     // Seller can cancel if not shipped? Or refund?
                     // Simplification: Allow cancel if not completed
                     if order.status == OrderStatus::Completed {
                         return Err(Error::InvalidOrderStatus);
                     }
                     order.status = OrderStatus::Cancelled;
                     // Logic to refund escrow would go here
                },
                _ => return Err(Error::Unauthorized),
             }
        } else if caller == order.buyer {
             match new_status {
                OrderStatus::Delivered => {
                    if order.status != OrderStatus::Shipped {
                        return Err(Error::InvalidOrderStatus);
                    }
                    order.status = OrderStatus::Delivered;
                },
                OrderStatus::Completed => {
                    // Buyer confirms completion -> Release Escrow
                    if order.status != OrderStatus::Delivered && order.status != OrderStatus::Shipped {
                         return Err(Error::InvalidOrderStatus);
                    }
                    order.status = OrderStatus::Completed;
                    order.is_escrow_released = true; 
                    // Note: Actual fund transfer should happen here
                },
                _ => return Err(Error::Unauthorized),
             }
        } else {
            // Admin check?
             return Err(Error::Unauthorized);
        }
        
        order.updated_at = e.ledger().timestamp();
        set_order(e, &order);
    }
    
    Self::extend_instance_ttl(e);
    Ok(())
}

pub fn batch_release_escrow(
    e: &Env,
    caller: Address,
    token: Address,
    order_ids: Vec<u64>,
) -> Result<(), Error> {
    caller.require_auth(); // Likely the buyer releasing to seller, or admin

    let token_client = token::Client::new(e, &token);

    for order_id in order_ids.iter() {
        let mut order = get_order(e, order_id).ok_or(Error::OrderNotFound)?;
        
        // Only buyer or admin can release?
        // Let's assume buyer calls this to finalize
        if caller != order.buyer {
            // Check admin
             let config = get_config(e).ok_or(Error::NotInitialized)?;
             if caller != config.admin {
                 return Err(Error::Unauthorized);
             }
        }

        if order.escrow_balance > 0 && order.status != OrderStatus::Completed {
             // Calculate fee
             let product = get_product(e, order.product_id).ok_or(Error::ProductNotFound)?;
             let category = get_category(e, product.category_id).ok_or(Error::CategoryNotFound)?;
             let fee_rate = category.commission_rate;
             let fee = (order.escrow_balance * fee_rate as u128) / 10000;
             let seller_amount = order.escrow_balance - fee;

             // Transfer funds to seller
             if seller_amount > 0 {
                 token_client.transfer(&e.current_contract_address(), &order.seller, &(seller_amount as i128));
             }
             
             // Track fees
             if fee > 0 {
                 add_fees(e, fee);
             }
             
             // Update order
             order.escrow_balance = 0;
             order.status = OrderStatus::Completed;
             order.updated_at = e.ledger().timestamp();
             set_order(e, &order);
             
             // Update seller stats
             let mut seller_data = get_seller(e, &order.seller).ok_or(Error::SellerNotFound)?;
             seller_data.total_sales += 1;
             seller_data.total_revenue += order.total_price;
             set_seller(e, &seller_data);
        }
    }
    
    extend_instance_ttl(e);
    Ok(())
}

pub fn batch_submit_rating(
    e: &Env,
    caller: Address,
    ratings: Vec<BatchSubmitRatingInput>,
) -> Result<(), Error> {
    caller.require_auth();

    for input in ratings.iter() {
        let order = get_order(e, input.order_id).ok_or(Error::OrderNotFound)?;
        
        // Only buyer can rate
        if order.buyer != caller {
            return Err(Error::Unauthorized);
        }
        
        // Can only rate if completed
        if order.status != OrderStatus::Completed {
             return Err(Error::InvalidOrderStatus);
        }
        
        // Basic rating validation
        if input.rating < 100 || input.rating > 500 {
             return Err(Error::InvalidInput);
        }

        // Update product rating
        let mut product = get_product(e, order.product_id).ok_or(Error::ProductNotFound)?;
        
        if product.rating == 0 {
            product.rating = input.rating;
        } else {
            product.rating = (product.rating + input.rating) / 2;
        }
        set_product(e, &product);
        
        // Update seller rating
        let mut seller = get_seller(e, &order.seller).ok_or(Error::SellerNotFound)?;
        
        if seller.rating == 0 {
            seller.rating = input.rating;
        } else {
            seller.rating = (seller.rating + input.rating) / 2;
        }
        set_seller(e, &seller);
    }
    
    extend_instance_ttl(e);
    Ok(())
}

fn extend_instance_ttl(e: &Env) {
     e.storage().instance().extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_AMOUNT);
}

#[macro_export]
macro_rules! impl_binary_node {
        ($ty:tt < $($param:ident),* >) => {
        impl< $($param),* > BinaryNode for $ty < $($param),* > {
            fn left(&self) -> *mut Self {
                self.left
            }

            fn right(&self) -> *mut Self {
                self.right
            }

            fn left_mut(&mut self) -> &mut *mut Self {
                &mut self.left
            }

            fn right_mut(&mut self) -> &mut *mut Self {
                &mut self.right
            }
        }
    };

    // 无泛型参数的类型
    ($ty:ty) => {
        impl BinaryNode for $ty {
            fn left(&self) -> *mut Self {
                self.left
            }

            fn right(&self) -> *mut Self {
                self.right
            }

            fn left_mut(&mut self) -> &mut *mut Self {
                &mut self.left
            }

            fn right_mut(&mut self) -> &mut *mut Self {
                &mut self.right
            }
        }
    };
}

#[macro_export]
macro_rules! impl_parent_node {
    ($ty:tt < $($param:ident),* >) => {
        impl< $($param),* > ParentRefNode for $ty < $($param),* > {
            fn parent(&self) -> *mut Self {
                self.parent
            }
            fn parent_mut(&mut self) -> &mut *mut Self {
                &mut self.parent
            }
        }
    };

    // 无泛型参数的类型
    ($ty:ty) => {
        impl ParentRefNode for $ty {
            fn parent(&self) -> *mut Self {
                self.parent
            }
            fn parent_mut(&mut self) -> &mut *mut Self {
                &mut self.parent
            }
        }
    };
}

#[macro_export]
macro_rules! impl_rb_node {
    ($ty:tt < $($param:ident),* >) => {
        impl< $($param),* > RedBlackNode for $ty < $($param),* > {
             fn color(&self) -> RBNodeColor {
                self.color
            }

            fn color_mut(&mut self) -> &mut RBNodeColor {
                &mut self.color
            }
        }
    };

    // 无泛型参数的类型
    ($ty:ty) => {
        impl RedBlackNode for $ty {
            fn color(&self) -> RBNodeColor {
                self.color
            }

            fn color_mut(&mut self) -> &mut RBNodeColor {
                &mut self.color
            }
        }
    };
}
